use passenger::passenger::Passenger;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use tokio::task::JoinHandle;

#[derive(Deserialize)]
struct PassengerData {
    id: u16,
    location: (u8, u8),
    destination: (u8, u8),
    card_number: u64,
}

#[actix::main]
async fn main() {
    let file_path = "../test_data/passengers.json";
    match create_passengers_from_json(file_path).await {
        Ok(passengers) => {
            for handle in passengers {
                match handle.await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{}", e);
        }
    }
}

/// Reads a JSON file and creates multiple `Passenger` instances.
///
/// # Arguments
///
/// * `file_path` - Path to the JSON file.
///
/// # Returns
///
/// A `Result` containing a vector of `Passenger` creation futures if successful, or an error string.
async fn create_passengers_from_json(file_path: &str) -> Result<Vec<JoinHandle<()>>, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);

    let passengers_data: Vec<PassengerData> =
        serde_json::from_reader(reader).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let mut handles = Vec::new();
    for data in passengers_data {
        let handle = actix::spawn(async move {
            if let Err(e) =
                Passenger::create(data.id, data.location, data.destination, data.card_number).await
            {
                eprintln!("{}", e);
            }
        });
        handles.push(handle);
    }

    Ok(handles)
}
