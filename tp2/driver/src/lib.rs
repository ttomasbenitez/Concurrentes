use crate::messages::{driver_messages::*, internal_driver_messages::*};
use actix::prelude::*;
use driver::{handle_driver_connection, handle_passenger_connection, Driver};
use messages::passenger_messages::{ConnectRes, PassengerMsg};
use serde::Serialize;
use std::io::BufRead;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::{TcpListener, TcpStream},
};

pub mod driver;
pub mod messages;

pub const MAX_DRIVERS: u16 = 5;
pub const DRIVER_PORT: u16 = 6000;
pub const PASSENGER_PORT: u16 = 7000;

/// Formats a message into a byte vector with a newline at the end.
///
/// This function takes a serializable message of type `M` (which implements
/// the `Serialize` trait), converts it into a JSON string, and appends a newline
/// character (`\n`) to the byte representation of the JSON string.
///
/// # Parameters
///
/// * `msg` - A serializable message of type `M` that will be converted to JSON.
///
/// # Returns
///
/// Returns a `Vec<u8>` containing the JSON representation of the message followed
/// by a newline character. If serialization fails, returns a byte vector with the error message.
pub fn format_msg<M: Serialize>(msg: M) -> Vec<u8> {
    if let Ok(json) = serde_json::to_string(&msg) {
        [json.as_bytes(), b"\n"].concat()
    } else {
        b"Error formatting message\n".to_vec()
    }
}

/// Calculates the Manhattan distance between two points in a 2D plane.
///
/// The Manhattan distance is calculated as the sum of the absolute differences
/// in the X and Y coordinates between two points `a` and `b`. This metric is commonly used
/// in grid-like environments, such as chessboards or city grids, where movement is restricted
/// to horizontal and vertical directions.
///
/// # Parameters
///
/// * `a` - A tuple `(x, y)` representing the coordinates of the first point.
/// * `b` - A tuple `(x, y)` representing the coordinates of the second point.
///
/// # Return
///
/// Returns a value of type `u8`, representing the Manhattan distance between the two points.
pub fn manhattan_distance(a: (u8, u8), b: (u8, u8)) -> u8 {
    ((a.0 as i16 - b.0 as i16).abs() + (a.1 as i16 - b.1 as i16).abs()) as u8
}

/// Handles incoming TCP connections and forwards them to a driver controller.
///
/// This function waits for new incoming TCP connections on the provided `TcpListener`
/// and, for each accepted connection, sends a `NewConnection` message to the `driver`
/// with the connection information for processing. The function operates asynchronously.
///
/// # Parameters
///
/// * `listener` - A `TcpListener` that is listening for incoming connections.
/// * `driver` - The address (`Addr<Driver>`) of the driver, which handles the incoming connections.
async fn handle_connections(listener: TcpListener, driver: Addr<Driver>) {
    while let Ok((stream, _)) = listener.accept().await {
        driver.do_send(NewConnection { stream });
    }
}

/// Attempts to establish a TCP connection with the driver to the right in the network.
///
/// This function attempts to connect to the closest driver in the system. It tries to connect
/// to a driver in a specific direction, moving through available drivers until it finds one
/// that is accessible. If the connection is successful, it returns a `TcpStream` and the ID
/// of the driver it connected to.
///
/// # Parameters
///
/// * `id` - The unique identifier of the current driver. This value is used to determine
///   the next driver to attempt a connection with.
///
/// # Returns
///
/// Returns an option containing a tuple:
/// - `Some((TcpStream, u16))`: A `TcpStream` for the established connection and the identifier
///   of the driver that was successfully connected to.
/// - `None`: If no connection could be established with any of the drivers.
async fn connect_to_right_driver(id: u16) -> Option<(TcpStream, u16)> {
    for i in 1..=MAX_DRIVERS - 1 {
        let driver_id = (id + i) % MAX_DRIVERS;
        let ip = format!("127.0.0.1:{}", DRIVER_PORT + driver_id);
        let stream = match TcpStream::connect(ip).await {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        return Some((stream, driver_id));
    }
    None
}

/// Handles messages received from a passenger over a TCP connection.
///
/// This function reads messages sent by a passenger, processes them, and forwards them
/// to a driver to take appropriate actions. If the message is a trip request,
/// the function sends that request for storage and makes a coordinate request
/// to the driver.
///
/// # Parameters
///
/// * `passenger_id` - The unique identifier of the passenger who sent the message.
/// * `reader` - The reader for the TCP connection with the passenger. Used to read incoming messages.
/// * `driver` - The address of the driver responsible for handling the request.
pub async fn handle_passenger_messages(
    passenger_id: u16,
    mut reader: ReadHalf<TcpStream>,
    driver: Addr<Driver>,
) {
    let mut buffer = [0; 2048];
    while let Ok(n) = reader.read(&mut buffer).await {
        if n == 0 {
            println!("Connection closed by passenger {}", passenger_id);
            return;
        }

        match serde_json::from_slice(&buffer[..n]) {
            Ok(DriverMsg::TripRequest(trip_request)) => {
                driver.do_send(StoreTrip {
                    id: passenger_id,
                    origin: trip_request.start,
                    destination: trip_request.end,
                });
                driver.do_send(CoordinatesRequest { passenger_id });
            }
            _ => {
                println!("Invalid message received from passenger {}", passenger_id);
            }
        }
    }
}

/// Receives and handles messages from another driver via a TCP connection.
///
/// This function reads the messages received from another driver, deserializes them, and processes
/// them based on the type of message. If the message is a disconnection or some other important action,
/// the function sends responses or takes appropriate actions.
///
/// # Parameters
///
/// * `_driver_id` - The unique identifier of the driver receiving the messages (not used directly in the function).
/// * `reader` - The reader for the TCP connection with the other driver.
/// * `driver` - The address of the driver handling the received messages.
/// * `from_left` - A boolean indicating if the message is from the driver to the left.
pub async fn recive_driver_messages(
    _driver_id: u16,
    mut reader: ReadHalf<TcpStream>,
    driver: Addr<Driver>,
    from_left: bool,
) {
    let mut bytes = [0; 2048];
    while let Ok(n) = reader.read(&mut bytes).await {
        if n == 0 {
            break;
        }

        for line in bytes[..n].lines().map_while(Result::ok) {
            match serde_json::from_str(&line) {
                Ok(DriverMsg::Disconnect) => {
                    driver.do_send(ConnectToRight);
                    return;
                }
                Ok(DriverMsg::NewCoordinator(new_coordinator)) => {
                    driver.do_send(NewCoordinator {
                        id: new_coordinator.id,
                    });
                }
                Ok(DriverMsg::CoordinatesRequest(coordinates_request)) => {
                    driver.do_send(CoordinatesRequest {
                        passenger_id: coordinates_request.passenger_id,
                    });
                }
                Ok(DriverMsg::CoordinatesResponse(coordinates_response)) => {
                    driver.do_send(CoordinatesResponse {
                        drivers_coordinates: coordinates_response.drivers_coordinates,
                        passenger_id: coordinates_response.passenger_id,
                    });
                }
                Ok(DriverMsg::OfferToDriver(offer)) => {
                    match driver.send(HandleOffer { offer }).await {
                        Ok(_) => {}
                        Err(e) => eprintln!("{:?}", e),
                    }
                }
                Ok(DriverMsg::TripResponse(trip_response)) => {
                    driver.do_send(trip_response);
                }
                Ok(DriverMsg::SendTripEnded(send_trip_ended)) => {
                    driver.do_send(send_trip_ended);
                }
                Ok(DriverMsg::DriverConnected(driver_connected)) => {
                    driver.do_send(driver_connected);
                }
                Ok(DriverMsg::UnresolvedTrip(unresolved_trip)) => {
                    driver.do_send(unresolved_trip);
                }
                _ => {}
            }
        }
    }
    if !from_left {
        driver.do_send(ConnectToRight);
    }
}

/// Handles a new passenger connection.
///
/// This function is responsible for managing a passenger connection, adding the passenger to the list
/// of connected passengers, sending messages related to unresolved messages, and starting a process
/// to handle the passenger's messages.
///
/// # Parameters
///
/// * `passenger_id` - The unique identifier of the passenger connecting.
/// * `reader` - The reader for the TCP connection to receive messages from the passenger.
/// * `writer` - The writer for the TCP connection to send messages to the passenger.
/// * `driver` - The address of the driver that will handle requests related to this passenger.
pub async fn handle_passenger_connections(
    passenger_id: u16,
    reader: ReadHalf<TcpStream>,
    writer: WriteHalf<TcpStream>,
    driver: Addr<Driver>,
) {
    println!("New passenger connected: {}", passenger_id);
    let _ = driver
        .send(AddPassengerStream {
            id: passenger_id,
            writer,
        })
        .await;
    driver.do_send(HandleUnresolvedMsgs { passenger_id });
    tokio::spawn(handle_passenger_messages(passenger_id, reader, driver));
}

/// Reads a message from a TCP stream.
///
/// This function reads data from the provided TCP stream and returns the received
/// content as a `Vec<u8>`. If the connection is closed, the function returns `None`.
///
/// # Parameters
///
/// * `reader` - A borrowed `ReadHalf<TcpStream>`, which is the reader of the TCP connection
///   from which messages will be read.
///
/// # Return
///
/// Returns an option with the bytes read from the stream:
/// - `Some(Vec<u8>)` if the data was read successfully.
/// - `None` if the connection was closed before reading data.
pub async fn read_message(reader: &mut ReadHalf<TcpStream>) -> Option<Vec<u8>> {
    let mut buffer = vec![0; 2048];
    let n = reader.read(&mut buffer).await.ok()?;
    if n == 0 {
        println!("Connection closed");
        return None;
    }
    Some(buffer[..n].to_vec())
}

/// Parses a series of messages from a byte buffer.
///
/// This function processes a byte buffer by splitting it into lines, deserializing
/// each line into a `DriverMsg` object, and collecting all successfully parsed messages
/// into a vector. If any line fails to deserialize, it is skipped.
///
/// # Parameters
///
/// * `buffer` - A slice of bytes (`&[u8]`) containing the messages to be parsed.
///
/// # Return
///
/// Returns an `Option<Vec<DriverMsg>>`:
/// - `Some(Vec<DriverMsg>)` if the messages are successfully parsed.
/// - `None` if no valid messages could be parsed.
pub fn parse_messages(buffer: &[u8]) -> Option<Vec<DriverMsg>> {
    buffer
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect::<Vec<_>>()
        .into()
}

/// Handles a connection request from either a driver or a passenger.
///
/// This function processes an incoming connection based on the type of the connection (either
/// a driver or a passenger). It responds with the appropriate message depending on whether
/// the connection request is valid or not. If successful, it returns the connection details.
///
/// # Parameters
///
/// * `connect` - A `Connect` message that contains the connection type (`from`) and other relevant
///   connection details.
/// * `coordinator_id` - An optional identifier for the coordinator driver.
/// * `id` - The identifier of the current driver.
/// * `reader` - A `ReadHalf<TcpStream>` to read incoming data from the connection.
/// * `writer` - A mutable `WriteHalf<TcpStream>` to write responses to the connection.
///
/// # Return
///
/// Returns an `Option` containing a tuple:
/// - `ConnectionType` indicating whether it's a driver or passenger connection.
/// - `ReadHalf<TcpStream>` for reading data from the connection.
/// - `WriteHalf<TcpStream>` for writing data to the connection.
/// - `u16` the connection ID.
/// - An optional `u16` representing the coordinator ID if the connection is from a driver.
pub async fn handle_connect(
    connect: Connect,
    coordinator_id: Option<u16>,
    id: u16,
    reader: ReadHalf<TcpStream>,
    mut writer: WriteHalf<TcpStream>,
) -> Option<(
    ConnectionType,
    ReadHalf<TcpStream>,
    WriteHalf<TcpStream>,
    u16,
    Option<u16>,
)> {
    match connect.from {
        ConnectionType::Driver => Some((
            ConnectionType::Driver,
            reader,
            writer,
            connect.id,
            connect.coordinator_id,
        )),
        ConnectionType::Passenger => match coordinator_id {
            Some(cid) if cid == id => {
                let message = PassengerMsg::ConnectRes(ConnectRes {
                    status: true,
                    leader_id: None,
                });
                println!("Passenger connected to coordinator");
                match writer.write_all(&format_msg(message)).await {
                    Ok(_) => Some((ConnectionType::Passenger, reader, writer, connect.id, None)),
                    Err(_) => None,
                }
            }
            _ => {
                let message = PassengerMsg::ConnectRes(ConnectRes {
                    status: false,
                    leader_id: None,
                });
                let _ = writer.write_all(&format_msg(message)).await;
                None
            }
        },
    }
}

/// Handles the result of a connection attempt, delegating to appropriate handlers
/// based on whether the connection is from a driver or a passenger.
///
/// This function processes the result of an attempted connection, which may involve a
/// driver or passenger. It calls the corresponding handler (`handle_driver_connection`
/// or `handle_passenger_connection`) based on the connection type.
///
/// # Parameters
///
/// * `result` - An `Option` containing a tuple with the connection details:
///   - `ConnectionType` specifying if the connection is from a driver or a passenger.
///   - `ReadHalf<TcpStream>` for reading data from the connection.
///   - `WriteHalf<TcpStream>` for writing data to the connection.
///   - `u16` the connection ID.
///   - An optional `u16` for the coordinator ID (only for drivers).
/// * `driver` - A mutable reference to the `Driver` struct, representing the driver’s state.
/// * `ctx` - A mutable reference to the `Context<Driver>`, used to send messages to the actor.
///
/// # Return
///
/// This function does not return any value. It delegates to the appropriate connection handler
/// based on the connection type (`Driver` or `Passenger`).
#[allow(clippy::type_complexity)]
pub fn handle_connection_result(
    result: Option<(
        ConnectionType,
        ReadHalf<TcpStream>,
        WriteHalf<TcpStream>,
        u16,
        Option<u16>,
    )>,
    driver: &mut Driver,
    ctx: &mut Context<Driver>,
) {
    if let Some((from, reader, writer, id, coordinator_id)) = result {
        match from {
            ConnectionType::Driver => {
                handle_driver_connection(id, coordinator_id, reader, writer, driver, ctx)
            }
            ConnectionType::Passenger => handle_passenger_connection(id, reader, writer, ctx),
        }
    }
}
