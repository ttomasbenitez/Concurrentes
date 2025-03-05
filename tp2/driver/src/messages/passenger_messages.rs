use super::driver_messages::TripResponse;
use actix::prelude::*;
use serde::{Deserialize, Serialize};

/// Enum to represent messages that can be sent to a passenger
#[derive(Debug, Serialize, Deserialize)]
pub enum PassengerMsg {
    TripResponse(TripResponse),
    TripStarted,
    TripEnded,
    ConnectRes(ConnectRes),
}

/// Message to indicate that a trip has started
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct TripStarted;

/// Message to indicate that a trip has ended
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct TripEnded {
    pub passenger_id: u16,
}

/// Message to indicate the result of a connection request
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct ConnectRes {
    pub status: bool,
    pub leader_id: Option<u16>,
}
