use actix::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Enum representing messages that can be sent to the driver
#[derive(Debug, Serialize, Deserialize)]
pub enum DriverMsg {
    TripRequest(TripRequest),
    Connect(Connect),
    Disconnect,
    NewCoordinator(NewCoordinator),
    CoordinatesRequest(CoordinatesRequest),
    CoordinatesResponse(CoordinatesResponse),
    OfferToDriver(OfferToDriver),
    TripResponse(TripResponse),
    SendTripEnded(SendTripEnded),
    DriverConnected(DriverConnected),
    UnresolvedTrip(UnresolvedTrip),
}

/// Message to request a trip
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct TripRequest {
    pub start: (u8, u8),
    pub end: (u8, u8),
}

/// Enum representing the type of connection
#[derive(Debug, Serialize, Deserialize)]
pub enum ConnectionType {
    Driver,
    Passenger,
}

/// Message to connect to a driver
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct Connect {
    pub from: ConnectionType,
    pub id: u16,
    pub coordinator_id: Option<u16>,
}

/// Message to announce a new coordinator
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct NewCoordinator {
    pub id: u16,
}

/// Message to request coordinates
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct CoordinatesRequest {
    pub passenger_id: u16,
}

/// Message to respond with coordinates
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct CoordinatesResponse {
    pub drivers_coordinates: HashMap<u16, (u8, u8)>,
    pub passenger_id: u16,
}

/// Message to offer a trip to a driver
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct OfferToDriver {
    pub driver_id: u16,
    pub origin: (u8, u8),
    pub destination: (u8, u8),
    pub passenger_id: u16,
}

/// Message to handle a trip response
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct HandleTripResponse {
    pub res: TripResponse,
    pub try_again: bool,
}

/// Enum representing the reason for declining a trip
#[derive(Debug, Serialize, Deserialize)]
pub enum DeclineReason {
    NotAccepted,
    DriversBusy,
}

/// Message to respond to a trip request
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct TripResponse {
    pub status: bool,
    pub reason: Option<DeclineReason>,
    pub passenger_id: u16,
    pub driver_id: u16,
}

/// Message to send a trip ended notification
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct SendTripEnded {
    pub passenger_id: u16,
}

/// Message to notify that a driver has connected
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct DriverConnected {
    pub driver_id: u16,
}

/// Message to notify that a trip is unresolved
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct UnresolvedTrip {
    pub passenger_id: u16,
    pub driver_id: u16,
}
