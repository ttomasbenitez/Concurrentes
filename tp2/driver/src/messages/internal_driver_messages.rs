use crate::OfferToDriver;
use actix::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::{io::WriteHalf, net::TcpStream};

/// Message to handle a closed connection
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct ConnectionClosed;

/// Message to set a new writer
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct SetNewWriter {
    pub writer: WriteHalf<TcpStream>,
}

/// Message to handle a new connection
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct NewConnection {
    pub stream: TcpStream,
}

/// Message to connect to the right driver
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct ConnectToRight;

/// Message to disconnect
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub new_stream: WriteHalf<TcpStream>,
    pub new_id: u16,
}

/// Message to send a message to the right driver
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct SendToRight {
    pub msg: Vec<u8>,
}

/// Message to send a message to the passenger
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct SendToPassenger {
    pub msg: Vec<u8>,
    pub passenger_id: u16,
}

/// Message to change the status of the driver
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct ChangeStatus;

/// Message to handle unresolved messages
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct HandleUnresolvedMsgs {
    pub passenger_id: u16,
}

/// Message to handle a trip offer
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct HandleOffer {
    pub offer: OfferToDriver,
}

/// Message to store a trip
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct StoreTrip {
    pub id: u16,
    pub origin: (u8, u8),
    pub destination: (u8, u8),
}

/// Message for the coordinator to select a driver
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct SelectDriver {
    pub passenger_id: u16,
}

/// Message to handle a passenger stream
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct AddPassengerStream {
    pub writer: WriteHalf<TcpStream>,
    pub id: u16,
}
