use actix::prelude::*;
use serde::{Deserialize, Serialize};

/// Enum to represent payment messages
#[derive(Debug, Serialize, Deserialize)]
pub enum PaymentMsg {
    ValidatePayment(ValidatePayment),
    MakePayment(MakePayment),
    ValidatePaymentResponse(ValidatePaymentResponse),
}

/// Message to validate payment
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct ValidatePayment {
    pub passenger_id: u16,
    pub card_number: u64,
}

/// Message to make payment
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct MakePayment {
    pub passenger_id: u16,
}

/// Enum to represent validation status
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub enum ValidationStatus {
    Success,
    Failure,
}

/// Message to validate payment response
#[derive(Message, Debug, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct ValidatePaymentResponse {
    pub status: ValidationStatus,
}
