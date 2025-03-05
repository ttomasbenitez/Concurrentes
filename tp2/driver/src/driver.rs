use crate::messages::{driver_messages::*, internal_driver_messages::*, passenger_messages::*};
use crate::{
    connect_to_right_driver, format_msg, handle_connect, handle_connection_result,
    handle_connections, handle_passenger_connections, manhattan_distance, parse_messages,
    read_message, recive_driver_messages, DRIVER_PORT,
};
use actix::prelude::*;
use actix::Actor;
use rand::Rng;
use std::collections::HashMap;
use tokio::task::JoinHandle;
use tokio::{
    io::{AsyncWriteExt, ReadHalf, WriteHalf},
    net::{TcpListener, TcpStream},
    time::sleep,
};

#[derive(Debug, Default)]
/// Represents the status of a driver.
enum DriverStatus {
    #[default]
    Available,
    Busy,
}

#[allow(clippy::type_complexity)]
#[derive(Debug, Default)]
/// Represents a driver in the system.
///
/// The `Driver` struct holds information about a driver’s status, position,
/// connections with other drivers, trips, and communication with passengers.
///
/// # Fields
///
/// * `id` - A unique identifier for the driver.
/// * `coordinates` - A tuple representing the current position of the driver
///   as (x, y) coordinates.
/// * `status` - The current status of the driver, which can be `Available`
///   or `Busy`.
/// * `free_drivers_position` - An optional map of other drivers' IDs and their
///   positions that are free and available for new trips.
/// * `driver_left_id` - An optional identifier for the driver to the left.
/// * `driver_left_write` - An optional write half of the TCP connection for
///   communication with the driver to the left.
/// * `driver_right_id` - An optional identifier for the driver to the right.
/// * `driver_right_write` - An optional write half of the TCP connection for
///   communication with the driver to the right.
/// * `coordinator_id` - An optional identifier for the coordinator driver in
///   the system.
/// * `passengers` - An optional `Arc<Mutex<HashMap<u16, WriteHalf<TcpStream>>>>`
///   holding a list of passengers and their respective TCP write end connections.
/// * `passengers_trips` - An optional map of passenger IDs to their trip
///   information, which includes the origin and destination coordinates.
/// * `trips_drivers_declined` - An optional map of declined trips, where
///   each passenger ID maps to a list of driver IDs who declined the trip.
/// * `unresolved_messages` - A map storing messages that could not be sent
///   to passengers when the connection was unavailable.
/// * `started_trips` - A map of trips that have started, where the key is the
///   driver ID and the value is the corresponding passenger ID.
pub struct Driver {
    id: u16,
    coordinates: (u8, u8),
    status: DriverStatus,
    free_drivers_position: Option<HashMap<u16, (u8, u8)>>,
    driver_left_id: Option<u16>,
    driver_left_write: Option<WriteHalf<TcpStream>>,
    driver_right_id: Option<u16>,
    driver_right_write: Option<WriteHalf<TcpStream>>,
    coordinator_id: Option<u16>,
    passengers: Option<HashMap<u16, WriteHalf<TcpStream>>>,
    passengers_trips: Option<HashMap<u16, ((u8, u8), (u8, u8))>>,
    trips_drivers_declined: Option<HashMap<u16, Vec<u16>>>,
    unresolved_messages: HashMap<u16, Vec<Vec<u8>>>,
    started_trips: HashMap<u16, u16>,
}

/// Implementation of the `Actor` trait for `Driver`.
impl Actor for Driver {
    type Context = Context<Self>;
}

impl Driver {
    /// Initializes and starts a new `Driver` instance, binds it to a TCP listener,
    /// and spawns an asynchronous task to handle incoming connections.
    ///
    /// # Arguments
    ///
    /// * `id` - A unique identifier for the driver. It is used to determine the port
    ///   number the driver will listen on.
    /// * `x_position` - The initial x-coordinate of the driver's location.
    /// * `y_position` - The initial y-coordinate of the driver's location.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing:
    /// * `Ok(JoinHandle<()>)` - A handle to the spawned asynchronous task that
    ///   processes incoming connections.
    /// * `Err(std::io::Error)` - An error if the TCP listener cannot bind to the
    ///   specified address.
    pub async fn start(
        id: u16,
        x_position: u8,
        y_position: u8,
    ) -> Result<JoinHandle<()>, std::io::Error> {
        let driver = Self {
            id,
            coordinates: (x_position, y_position),
            status: DriverStatus::Available,
            ..Default::default()
        };
        let ip = format!("127.0.0.1:{}", DRIVER_PORT + id);
        let tcp_listener = match TcpListener::bind(ip).await {
            Ok(listener) => listener,
            Err(e) => {
                return Err(e);
            }
        };
        let driver_addr = driver.start();
        let task_handle = tokio::spawn(handle_connections(tcp_listener, driver_addr.clone()));
        driver_addr.do_send(ConnectToRight);
        Ok(task_handle)
    }
}

/// Handles the `CoordinatesResponse` message in the `Driver` actor.
///
/// If the driver is the coordinator, it updates the `free_drivers_position` field with the received coordinates
/// and sends a `SelectDriver` message to itself with the passenger ID.
/// If the driver is not the coordinator and its status is `Available`, it adds its coordinates to the list and
/// forwards the message to the right driver with `SendToRight`.
impl Handler<CoordinatesResponse> for Driver {
    type Result = ();

    fn handle(&mut self, mut msg: CoordinatesResponse, ctx: &mut Context<Self>) -> Self::Result {
        let coordinator_id = match self.coordinator_id {
            Some(id) => id,
            None => return,
        };

        if self.id == coordinator_id {
            self.free_drivers_position = Some(msg.drivers_coordinates);
            ctx.address().do_send(SelectDriver {
                passenger_id: msg.passenger_id,
            })
        } else if self.id != coordinator_id {
            let _ = match self.status {
                DriverStatus::Available => {
                    msg.drivers_coordinates.insert(self.id, self.coordinates)
                }
                _ => None,
            };
            let message = format_msg(DriverMsg::CoordinatesResponse(msg));
            ctx.address().do_send(SendToRight { msg: message });
        }
    }
}

/// Handles the `StoreTrip` message in the `Driver` actor.
///
/// This handler processes a trip request for a passenger. It manages the passenger's trip details, including
/// the origin and destination. It also handles updates to the list of trips and declined trips for the driver.
///
/// ### Behavior:
/// - If the passenger is already in a trip (i.e., their ID is in `passengers_trips`), it logs a message and does nothing.
/// - If the passenger is not in a trip, it adds the trip to the `passengers_trips` field. If this is the first trip,
///   a new `HashMap` is created to store the trip details.
/// - If there are any drivers who have previously declined the passenger's trip, the passenger is removed from the
///   `trips_drivers_declined` list.
impl Handler<StoreTrip> for Driver {
    type Result = ();

    fn handle(&mut self, msg: StoreTrip, _ctx: &mut Context<Self>) -> Self::Result {
        let passengers_trips = self.passengers_trips.take();
        if let Some(mut trips) = passengers_trips {
            if trips.contains_key(&msg.id) {
                println!("passenger {} already in a trip!", msg.id);
                return;
            }
            trips.insert(msg.id, (msg.origin, msg.destination));
            self.passengers_trips = Some(trips);
        } else {
            let mut new_trips = HashMap::new();
            new_trips.insert(msg.id, (msg.origin, msg.destination));
            self.passengers_trips = Some(new_trips);
        }
        if let Some(trips_drivers_declined) = &self.trips_drivers_declined {
            if trips_drivers_declined.contains_key(&msg.id) {
                let mut declined_drivers = trips_drivers_declined.clone();
                declined_drivers.remove(&msg.id);
                self.trips_drivers_declined = Some(declined_drivers);
            }
        }
    }
}

/// Handles the `SelectDriver` message in the `Driver` actor.
///
/// This handler selects the closest available driver for a passenger's trip based on the Manhattan distance to the passenger's origin.
/// It also checks if the passenger's trip has been declined by any drivers and skips those drivers during the selection process.
///
/// ### Behavior:
/// - If there are any declined drivers for the passenger, they are excluded from the selection.
/// - It looks for the passenger's trip details (origin and destination) and selects the closest available driver from `free_drivers_position`.
/// - If a driver is found, an `OfferToDriver` message is sent to the selected driver with trip details (passenger ID, origin, and destination).
/// - If no driver is found, a `TripResponse` message is sent to the passenger indicating the trip was declined, with a reason depending on whether any driver has declined the trip.
impl Handler<SelectDriver> for Driver {
    type Result = ();

    fn handle(&mut self, msg: SelectDriver, ctx: &mut Context<Self>) -> Self::Result {
        let declined_drivers = match &self.trips_drivers_declined {
            Some(declined_drivers) => declined_drivers
                .get(&msg.passenger_id)
                .unwrap_or(&Vec::new())
                .clone(),
            None => Vec::new(),
        };

        if let (Some(drivers), Some(trips)) = (&self.free_drivers_position, &self.passengers_trips)
        {
            if let Some(&(start, _)) = trips.get(&msg.passenger_id) {
                let closest_driver = drivers
                    .iter()
                    .filter(|(&driver_id, _)| !declined_drivers.contains(&driver_id))
                    .min_by_key(|(_, &driver_pos)| manhattan_distance(driver_pos, start));
                let addr = ctx.address();
                if let Some(closest_driver) = closest_driver {
                    println!("Closest driver {:?}", closest_driver);
                    addr.do_send(OfferToDriver {
                        passenger_id: msg.passenger_id,
                        driver_id: *closest_driver.0,
                        origin: start,
                        destination: trips.get(&msg.passenger_id).unwrap_or(&((0, 0), (0, 0))).1,
                    });
                } else {
                    let decline_msg = format_msg(DriverMsg::TripResponse(TripResponse {
                        status: false,
                        reason: match declined_drivers.len() {
                            0 => Some(DeclineReason::DriversBusy),
                            _ => Some(DeclineReason::NotAccepted),
                        },
                        passenger_id: msg.passenger_id,
                        driver_id: self.id,
                    }));
                    let trips = self.passengers_trips.take();
                    if let Some(mut trips) = trips {
                        trips.remove(&msg.passenger_id);
                        self.passengers_trips = Some(trips);
                    }
                    addr.do_send(SendToPassenger {
                        msg: decline_msg,
                        passenger_id: msg.passenger_id,
                    });
                }
            }
        }
    }
}

/// Handles the `OfferToDriver` message in the `Driver` actor.
///
/// This handler processes the offer message sent to the driver. If the driver is the intended recipient,
/// it handles the offer by sending a `HandleOffer` message. If the driver is not the intended recipient,
/// the offer is forwarded to the right driver.
///
/// ### Behavior:
/// - If the `driver_id` in the offer matches the current driver's ID, it processes the offer by sending a `HandleOffer` message to itself.
/// - If the `driver_id` does not match, it forwards the offer message to the right driver by sending a `SendToRight` message.
impl Handler<OfferToDriver> for Driver {
    type Result = ();

    fn handle(&mut self, msg: OfferToDriver, ctx: &mut Context<Self>) -> Self::Result {
        if msg.driver_id == self.id {
            println!("Offer received");
            ctx.address().do_send(HandleOffer { offer: msg });
            return;
        }
        let msg = format_msg(DriverMsg::OfferToDriver(msg));
        ctx.address().do_send(SendToRight { msg });
    }
}

/// Handles the `NewConnection` message in the `Driver` actor.
///
/// This handler processes a new connection request from either a driver or a passenger. It reads the incoming stream,
/// parses the received messages, and handles them accordingly. The connection is established based on the type (driver or passenger),
/// and specific actions are taken for each type.
///
/// ### Behavior:
/// - **Driver Connection**:
///   - If the connection is from a driver:
///     - If this driver is the coordinator, updates the `coordinator_id` and sends a `NewCoordinator` message.
///     - Disconnects any previous driver if necessary and establishes a connection to the right driver.
///     - Starts a new task to receive messages from the connected driver.
/// - **Passenger Connection**:
///   - If the connection is from a passenger:
///     - If there is no coordinator, the passenger is rejected.
///     - If there is a coordinator and the current driver is the coordinator, the connection is accepted and a message is sent.
///     - If the current driver is not the coordinator, the passenger is rejected.
impl Handler<NewConnection> for Driver {
    type Result = ();

    fn handle(&mut self, msg: NewConnection, _ctx: &mut Context<Self>) -> Self::Result {
        let coordinator_id = self.coordinator_id;
        let id = self.id;
        let (mut reader, writer) = tokio::io::split(msg.stream);
        let addr = _ctx.address();

        async move {
            let buffer = read_message(&mut reader).await?;
            let messages = parse_messages(&buffer)?;

            for message in messages.into_iter().rev() {
                match message {
                    DriverMsg::Connect(connect) => {
                        return handle_connect(connect, coordinator_id, id, reader, writer).await;
                    }
                    DriverMsg::UnresolvedTrip(unresolved_trip) => {
                        addr.do_send(unresolved_trip);
                    }
                    DriverMsg::DriverConnected(driver_connected) => {
                        addr.do_send(driver_connected);
                    }
                    _ => println!("Invalid message"),
                }
            }
            None
        }
        .into_actor(self)
        .map(handle_connection_result)
        .wait(_ctx);
    }
}

/// Handles the `Disconnect` message in the `Driver` actor.
///
/// This handler processes a disconnection request from a driver, sending a `Disconnect` message to the driver
/// that is being disconnected. If the driver was previously connected to the current driver, the handler updates
/// the state of the `Driver` to reflect the new connection.
///
/// ### Behavior:
/// - If the driver has a `driver_left_write` (i.e., a previously connected driver):
///   - Sends a `Disconnect` message to that driver.
///   - Updates the current driver's state by replacing `driver_left_write` and `driver_left_id` with the new connection information.
/// - If no previous connection exists (`driver_left_write` is `None`):
///   - Updates the driver's state with the new stream and ID.
impl Handler<Disconnect> for Driver {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _ctx: &mut Context<Self>) -> Self::Result {
        let left_write = self.driver_left_write.take();
        async move {
            if let Some(mut left_writer) = left_write {
                let message = format_msg(DriverMsg::Disconnect);
                match left_writer.write_all(&message).await {
                    Ok(_) => {}
                    Err(_) => println!("Error sending disconnect message"),
                }
                return (msg.new_stream, msg.new_id);
            }
            (msg.new_stream, msg.new_id)
        }
        .into_actor(self)
        .map(|res, driver, _| {
            driver.driver_left_write = Some(res.0);
            driver.driver_left_id = Some(res.1);
            println!(
                "|{:?}|-|D{:?}|-|{:?}|",
                driver.driver_left_id, driver.id, driver.driver_right_id
            );
        })
        .wait(_ctx);
    }
}

/// Handles the `ConnectToRight` message in the `Driver` actor.
///
/// This handler establishes a connection with the driver to the right. It attempts to connect to the right driver
/// and updates the driver's state accordingly. If no connection is possible, it designates the current driver as
/// the new coordinator.
///
/// ### Behavior:
/// - **If connection to the right driver is successful**:
///   - Splits the stream and saves the connection details (`driver_right_id` and `driver_right_write`).
///   - Checks if the current driver should be the coordinator or if the right driver becomes the new coordinator.
///   - Sends a `Connect` message to the right driver with updated coordinator information.
///   - Starts a new task to receive messages from the right driver.
/// - **If no connection is established** (i.e., no right driver exists):
///   - Sets the current driver as the new coordinator.
impl Handler<ConnectToRight> for Driver {
    type Result = ();

    fn handle(&mut self, _msg: ConnectToRight, ctx: &mut Context<Self>) -> Self::Result {
        let id = self.id;
        let addr = ctx.address();
        async move { connect_to_right_driver(id).await }
            .into_actor(self)
            .map(|res, driver, _| match res {
                Some((stream, driver_id)) => {
                    let (read, write) = tokio::io::split(stream);
                    driver.driver_right_write = Some(write);
                    driver.driver_right_id = Some(driver_id);
                    let mut coordinator_id;
                    if driver_id < driver.id {
                        coordinator_id = Some(driver.id);
                    } else {
                        if let Some(current_coordinator) = driver.coordinator_id {
                            if driver.id == current_coordinator {
                                coordinator_id = Some(driver_id);
                                driver.coordinator_id = coordinator_id;
                            }
                        }
                        coordinator_id = driver.coordinator_id;
                    }
                    println!("Coordinator id {:?}", coordinator_id);
                    println!(
                        "|{:?}|-|D{:?}|-|{:?}|",
                        driver.driver_left_id, driver.id, driver.driver_right_id
                    );
                    let msg = format_msg(DriverMsg::Connect(Connect {
                        from: ConnectionType::Driver,
                        id: driver.id,
                        coordinator_id,
                    }));
                    addr.do_send(SendToRight { msg });
                    tokio::spawn(recive_driver_messages(driver_id, read, addr, false));
                }
                None => {
                    driver.coordinator_id = Some(driver.id);
                    driver.driver_right_id = None;
                    driver.driver_right_write = None;
                    driver.driver_left_id = None;
                    driver.driver_left_write = None;
                    println!("Coordinator id {:?}", driver.coordinator_id);
                    println!(
                        "|{:?}|-|D{:?}|-|{:?}|",
                        driver.driver_left_id, driver.id, driver.driver_right_id
                    );
                }
            })
            .wait(ctx);
    }
}

/// Handles the `NewCoordinator` message in the `Driver` actor.
///
/// This handler updates the coordinator ID for the driver. If the new coordinator is not the current driver,
/// it propagates the `NewCoordinator` message to the next driver in the chain.
///
/// ### Behavior:
/// - Sets the driver's `coordinator_id` to the new coordinator ID.
/// - If the new coordinator is not the current driver, sends the `NewCoordinator` message to the right driver.
impl Handler<NewCoordinator> for Driver {
    type Result = ();

    fn handle(&mut self, msg: NewCoordinator, ctx: &mut Context<Self>) -> Self::Result {
        self.coordinator_id = Some(msg.id);
        if msg.id != self.id {
            let msg = format_msg(DriverMsg::NewCoordinator(msg));
            ctx.address().do_send(SendToRight { msg });
        }
    }
}

/// Handles the `CoordinatesRequest` message in the `Driver` actor.
///
/// This handler responds to a request for the driver's coordinates. If the driver is available and has no right-side connection,
/// it sends an `OfferToDriver` message. Otherwise, it collects the driver's coordinates and forwards them to the next driver.
///
/// ### Behavior:
/// - If the driver has no right-side connection, sends an `OfferToDriver` message with the driver's ID and coordinates.
/// - If the driver is available and has a right-side connection, collects and forwards its coordinates to the next driver.
impl Handler<CoordinatesRequest> for Driver {
    type Result = ();

    fn handle(&mut self, msg: CoordinatesRequest, ctx: &mut Context<Self>) -> Self::Result {
        let id = self.id;
        let coordinates = self.coordinates;
        let addr = ctx.address();
        let add_coordinates = matches!(self.status, DriverStatus::Available);
        if self.driver_right_write.is_none() {
            addr.do_send(HandleOffer {
                offer: OfferToDriver {
                    driver_id: id,
                    origin: coordinates,
                    destination: (0, 0),
                    passenger_id: msg.passenger_id,
                },
            });
        } else {
            let mut drivers_coordinates = HashMap::new();
            if add_coordinates {
                drivers_coordinates.insert(id, coordinates);
            }
            let msg = format_msg(DriverMsg::CoordinatesResponse(CoordinatesResponse {
                drivers_coordinates,
                passenger_id: msg.passenger_id,
            }));
            addr.do_send(SendToRight { msg });
        }
    }
}

/// Handles the `HandleOffer` message in the `Driver` actor.
///
/// This handler processes an offer from another driver to take a passenger. The driver decides whether to accept or decline
/// the offer based on its status and a probability. If the offer is accepted, the driver updates its status to "Busy".
/// If the driver is not the coordinator, the offer is forwarded to the next driver.
///
/// ### Behavior:
/// - If the driver is the one who received the offer:
///   - Accepts or declines the offer randomly if not already busy, or based on its current status.
///   - If the driver is the coordinator, it updates the trip list and sends a response to the passenger.
///   - If the driver accepts the offer, updates its status to "Busy" and sends a `TripEnded` message.
/// - If the driver is not the recipient of the offer, it forwards the offer to the next driver.
impl Handler<HandleOffer> for Driver {
    type Result = ();

    fn handle(&mut self, msg: HandleOffer, ctx: &mut Context<Self>) -> Self::Result {
        let addr = ctx.address();
        if msg.offer.driver_id == self.id {
            let mut rng = rand::thread_rng();
            let mut status = rng.gen_bool(0.5);
            if matches!(self.status, DriverStatus::Busy) {
                status = false;
            }
            if self.id == self.coordinator_id.unwrap_or(self.id) {
                let trips = self.passengers_trips.take();
                if let Some(mut trips) = trips {
                    trips.remove(&msg.offer.passenger_id);
                    self.passengers_trips = Some(trips);
                }
                addr.do_send(SendToPassenger {
                    passenger_id: msg.offer.passenger_id,
                    msg: format_msg(DriverMsg::TripResponse(TripResponse {
                        status,
                        reason: None,
                        passenger_id: msg.offer.passenger_id,
                        driver_id: self.id,
                    })),
                });
            } else {
                let msg = format_msg(DriverMsg::TripResponse(TripResponse {
                    status,
                    reason: None,
                    passenger_id: msg.offer.passenger_id,
                    driver_id: self.id,
                }));
                addr.do_send(SendToRight { msg });
            }
            if status {
                self.status = DriverStatus::Busy;
                addr.do_send(TripEnded {
                    passenger_id: msg.offer.passenger_id,
                });
            }
        } else {
            addr.do_send(SendToRight {
                msg: format_msg(DriverMsg::OfferToDriver(msg.offer)),
            });
        }
    }
}

/// Handles the `TripResponse` message in the `Driver` actor.
///
/// This handler processes a response to a trip offer from a passenger. It determines whether the offer was accepted or declined.
/// If declined, it adds the passenger to the list of declined trips and sends a `CoordinatesRequest` to get new driver coordinates.
/// If accepted, it records the trip as started and sends a confirmation to the passenger.
///
/// ### Behavior:
/// - If the driver is not the coordinator:
///   - Forwards the response to the next driver.
/// - If the driver is the coordinator:
///   - If the trip was declined, adds the passenger to the list of declined trips and sends a `CoordinatesRequest`.
///   - If the trip was accepted, records the trip and notifies the passenger of the acceptance.
impl Handler<TripResponse> for Driver {
    type Result = ();

    fn handle(&mut self, msg: TripResponse, ctx: &mut Context<Self>) -> Self::Result {
        if self.id != self.coordinator_id.unwrap_or(self.id) {
            ctx.address().do_send(SendToRight {
                msg: format_msg(DriverMsg::TripResponse(msg)),
            });
            return;
        }
        let passenger_id = msg.passenger_id;
        let addr = ctx.address();
        if !msg.status {
            self.trips_drivers_declined
                .get_or_insert_with(HashMap::new)
                .entry(passenger_id)
                .or_default()
                .push(passenger_id);
            addr.do_send(CoordinatesRequest { passenger_id });
            return;
        }
        self.started_trips.insert(msg.driver_id, passenger_id);
        addr.do_send(SendToPassenger {
            msg: format_msg(PassengerMsg::TripResponse(msg)),
            passenger_id,
        });
    }
}

/// Handles the `TripEnded` message in the `Driver` actor.
///
/// This handler processes the end of a trip, performing the following actions:
/// - Pauses for 10 seconds for simulating the trip before notifying the actor to change its status.
/// - If the driver is the coordinator, it sends a `SendTripEnded` message with the passenger ID.
/// - If the driver is not the coordinator, it forwards the `SendTripEnded` message to the next driver.
///
/// ### Behavior:
/// - After waiting for 10 seconds, the driver changes its status and sends the appropriate `SendTripEnded` message.
/// - If the driver is the coordinator, the trip end message is sent directly to the passenger.
/// - If the driver is not the coordinator, the message is forwarded to the next driver in the chain.
impl Handler<TripEnded> for Driver {
    type Result = ();

    fn handle(&mut self, msg: TripEnded, ctx: &mut Context<Self>) -> Self::Result {
        let id = self.id;
        let coordinator_id = self.coordinator_id.unwrap_or(self.id);
        let addr = ctx.address();
        tokio::spawn(async move {
            println!("Starting trip");
            sleep(std::time::Duration::from_secs(10)).await;
            addr.do_send(ChangeStatus);
            println!("Trip ended!");
            if id == coordinator_id {
                addr.do_send(SendTripEnded {
                    passenger_id: msg.passenger_id,
                });
                return;
            }
            addr.do_send(SendToRight {
                msg: format_msg(DriverMsg::SendTripEnded(SendTripEnded {
                    passenger_id: msg.passenger_id,
                })),
            });
        });
    }
}

/// Handles the `ChangeStatus` message in the `Driver` actor.
///
/// This handler toggles the driver's status between `Busy` and `Available`.
/// If the driver is currently `Busy`, it changes the status to `Available`.
/// If the driver is `Available`, it changes the status to `Busy`.
impl Handler<ChangeStatus> for Driver {
    type Result = ();

    fn handle(&mut self, _msg: ChangeStatus, _ctx: &mut Context<Self>) -> Self::Result {
        if matches!(self.status, DriverStatus::Busy) {
            self.status = DriverStatus::Available;
        } else {
            self.status = DriverStatus::Busy;
        }
    }
}

/// Handles the `SendToRight` message in the `Driver` actor.
///
/// This handler sends the provided message (`msg`) to the driver on the right
/// (the connected neighbor) by writing it to the `driver_right_write` stream.
/// If the `driver_right_write` stream is available, it sends the message and
/// restores the writer. If an error occurs while sending, it logs the error.
impl Handler<SendToRight> for Driver {
    type Result = ();

    fn handle(&mut self, msg: SendToRight, _ctx: &mut Context<Self>) -> Self::Result {
        let right_writer = self.driver_right_write.take();
        async move {
            if let Some(mut right_writer) = right_writer {
                match right_writer.write_all(&msg.msg).await {
                    Ok(_) => {}
                    Err(_) => println!("Error sending OfferToDriver message"),
                }
                return Some(right_writer);
            }
            right_writer
        }
        .into_actor(self)
        .map(|res, driver, _| {
            driver.driver_right_write = res;
        })
        .wait(_ctx);
    }
}

/// Handles the `SendToPassenger` message in the `Driver` actor.
///
/// This handler attempts to send a message to a passenger. If the passenger's writer
/// is available in the `passengers` list, it writes the message to the passenger. If the
/// passenger is not found, or if there is an error sending the message, the message is saved
/// for later delivery. If the passenger list is not initialized, the message is also saved.
impl Handler<SendToPassenger> for Driver {
    type Result = ();

    fn handle(&mut self, msg: SendToPassenger, ctx: &mut Context<Self>) -> Self::Result {
        let passenger_id: u16 = msg.passenger_id;
        let mut passengers = match self.passengers.take() {
            Some(passengers) => passengers,
            None => {
                println!("Passengers not initialized, saving message");
                self.unresolved_messages
                    .entry(passenger_id)
                    .or_default()
                    .push(msg.msg);
                return;
            }
        };
        let msg_to_save = msg.msg.clone();
        async move {
            let mut passenger_writer = match passengers.remove(&msg.passenger_id) {
                Some(writer) => writer,
                None => {
                    println!("Passenger not found, saving message");
                    return (Err(()), passengers);
                }
            };
            let result = match passenger_writer.write_all(&msg.msg).await {
                Ok(_) => Ok(()),
                Err(_) => {
                    println!("Error sending message to passenger, saving message");
                    Err(())
                }
            };
            passengers.insert(passenger_id, passenger_writer);
            (result, passengers)
        }
        .into_actor(self)
        .map(move |res, driver, _| {
            if res.0.is_err() {
                driver
                    .unresolved_messages
                    .entry(passenger_id)
                    .or_default()
                    .push(msg_to_save);
            }
            driver.passengers = Some(res.1);
        })
        .wait(ctx);
    }
}

/// Handles the `SendTripEnded` message in the `Driver` actor.
///
/// This handler processes the end of a trip by updating the driver's trip information.
/// If the current driver is the coordinator, it performs the following actions:
/// - Removes the driver-passenger association from the `started_trips` map.
/// - Removes the passenger from the `passengers_trips` map.
/// - Sends a `TripEnded` message to the passenger to notify them that the trip has ended.
///   If the current driver is not the coordinator, the message is forwarded to the right driver.
impl Handler<SendTripEnded> for Driver {
    type Result = ();

    fn handle(&mut self, msg: SendTripEnded, ctx: &mut Context<Self>) -> Self::Result {
        if self.id != self.coordinator_id.unwrap_or(self.id) {
            ctx.address().do_send(SendToRight {
                msg: format_msg(DriverMsg::SendTripEnded(msg)),
            });
            return;
        }
        let addr = ctx.address();
        let key = self.started_trips.iter().find_map(|(k, v)| {
            if *v == msg.passenger_id {
                Some(*k)
            } else {
                None
            }
        });
        if let Some(driver_id) = key {
            self.started_trips.remove(&driver_id);
        }
        let trips = self.passengers_trips.take();
        if let Some(mut trips) = trips {
            trips.remove(&msg.passenger_id);
            self.passengers_trips = Some(trips);
        }
        addr.do_send(SendToPassenger {
            msg: format_msg(PassengerMsg::TripEnded),
            passenger_id: msg.passenger_id,
        });
    }
}

/// Handles the `HandleUnresolvedMsgs` message in the `Driver` actor.
///
/// This handler processes any unresolved messages that were saved while a passenger
/// was not available or not connected. The unresolved messages are retrieved and
/// sent to the passenger once they are connected.
///
/// It removes the unresolved messages from the `unresolved_messages` map for the
/// given `passenger_id` and sends the messages to the corresponding passenger.
impl Handler<HandleUnresolvedMsgs> for Driver {
    type Result = ();

    fn handle(&mut self, msg: HandleUnresolvedMsgs, ctx: &mut Context<Self>) -> Self::Result {
        let messages = self
            .unresolved_messages
            .remove(&msg.passenger_id)
            .unwrap_or_default();
        for message in messages {
            ctx.address().do_send(SendToPassenger {
                msg: message,
                passenger_id: msg.passenger_id,
            });
        }
    }
}

/// Handles the `DriverConnected` message in the `Driver` actor.
///
/// This handler processes the event when a driver connects. It checks if the current driver
/// is the coordinator and if there are unresolved trips associated with the newly connected driver.
///
/// If the current driver is not the coordinator, it forwards the message to the right driver.
/// If there are unresolved trips for the connected driver, it sends those unresolved trips to the right driver.
impl Handler<DriverConnected> for Driver {
    type Result = ();

    fn handle(&mut self, msg: DriverConnected, ctx: &mut Context<Self>) -> Self::Result {
        let driver_id = msg.driver_id;
        if self.id != self.coordinator_id.unwrap_or(self.id) {
            ctx.address().do_send(SendToRight {
                msg: format_msg(DriverMsg::DriverConnected(msg)),
            });
        }
        if self.started_trips.contains_key(&driver_id) {
            let passenger_id = match self.started_trips.get(&driver_id) {
                Some(id) => id,
                None => return,
            };
            ctx.address().do_send(SendToRight {
                msg: format_msg(DriverMsg::UnresolvedTrip(UnresolvedTrip {
                    passenger_id: *passenger_id,
                    driver_id,
                })),
            });
        }
    }
}

/// Handles the `UnresolvedTrip` message in the `Driver` actor.
///
/// This handler processes the event when an unresolved trip message is received for a driver.
/// If the current driver is not the driver in the `UnresolvedTrip` message, the message is forwarded
/// to the right driver. Otherwise, the trip is marked as ended by sending a `SendTripEnded` message.
///
/// # Flow:
/// - If the driver receiving the message is not the one it is intended for, the message is forwarded
///   to the appropriate driver.
/// - If the driver is the intended one, the trip is ended and a `SendTripEnded` message is sent.
impl Handler<UnresolvedTrip> for Driver {
    type Result = ();

    fn handle(&mut self, msg: UnresolvedTrip, ctx: &mut Context<Self>) -> Self::Result {
        let addr = ctx.address();
        if self.id != msg.driver_id {
            addr.do_send(SendToRight {
                msg: format_msg(DriverMsg::UnresolvedTrip(msg)),
            });
            return;
        }
        println!("Sending trip ended.");
        addr.do_send(SendTripEnded {
            passenger_id: msg.passenger_id,
        });
    }
}
/// Handles the `AddPassengerStream` message in the `Driver` actor.
///
/// This handler processes a new passenger connection by updating the driver's state and adding the passenger's writer to the `passengers` list.
impl Handler<AddPassengerStream> for Driver {
    type Result = ();

    fn handle(&mut self, msg: AddPassengerStream, _ctx: &mut Context<Self>) -> Self::Result {
        let mut passengers = self.passengers.take().unwrap_or_default();
        passengers.insert(msg.id, msg.writer);
        self.passengers = Some(passengers);
    }
}

/// Handles a driver connection by updating the driver's state and sending messages to the right driver.
/// It also spawns a task to handle driver messages.
///
/// # Arguments
/// * `id` - The ID of the driver connecting.
/// * `coordinator_id` - An optional coordinator ID for the driver, if available.
/// * `reader` - The read half of the `TcpStream` to receive messages from the driver.
/// * `writer` - The write half of the `TcpStream` to send messages to the driver.
/// * `driver` - A mutable reference to the `Driver` struct representing the current driver.
/// * `ctx` - The actor context, providing access to the actor's address for message sending.
pub fn handle_driver_connection(
    id: u16,
    coordinator_id: Option<u16>,
    reader: ReadHalf<TcpStream>,
    writer: WriteHalf<TcpStream>,
    driver: &mut Driver,
    ctx: &mut Context<Driver>,
) {
    let addr = ctx.address();
    if let Some(coordinator_id) = coordinator_id {
        driver.coordinator_id = Some(coordinator_id);
        println!("Coordinator id {:?}", driver.coordinator_id);
        let msg = format_msg(DriverMsg::NewCoordinator(NewCoordinator {
            id: coordinator_id,
        }));
        addr.do_send(SendToRight { msg });
    }
    if driver.driver_left_write.is_some() {
        addr.do_send(Disconnect {
            new_id: id,
            new_stream: writer,
        });
    } else {
        driver.driver_left_id = Some(id);
        driver.driver_left_write = Some(writer);
        println!(
            "|{:?}|-|D{:?}|-|{:?}|",
            driver.driver_left_id, driver.id, driver.driver_right_id
        );
    }
    if driver.driver_right_id.is_none() {
        addr.do_send(ConnectToRight);
    } else {
        addr.do_send(SendToRight {
            msg: format_msg(DriverMsg::DriverConnected(DriverConnected {
                driver_id: id,
            })),
        });
    }
    tokio::spawn(recive_driver_messages(id, reader, addr, true));
}

/// Handles a new passenger connection by updating the driver's state and spawning a task to handle passenger connections.
///
/// # Arguments
/// * `id` - The ID of the passenger connecting.
/// * `reader` - The read half of the `TcpStream` to receive messages from the passenger.
/// * `writer` - The write half of the `TcpStream` to send messages to the passenger.
/// * `ctx` - The actor context, providing access to the actor's address for message sending.
pub fn handle_passenger_connection(
    id: u16,
    reader: ReadHalf<TcpStream>,
    writer: WriteHalf<TcpStream>,
    ctx: &mut Context<Driver>,
) {
    tokio::spawn(handle_passenger_connections(
        id,
        reader,
        writer,
        ctx.address(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_rt::test]
    async fn test_un_conductor_solo_es_coordinador() {
        let _driver = Driver::start(1, 80, 20).await.unwrap();
    }
}
