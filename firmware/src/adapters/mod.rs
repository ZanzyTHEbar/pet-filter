//! Adapters — concrete implementations of the hexagonal port traits.
//!
//! | Adapter        | Implements         | Connects to              |
//! |----------------|--------------------|--------------------------|
//! | `ble`          | ProvisioningPort   | NimBLE GATT server       |
//! | `hardware`     | SensorPort         | ESP32 ADC, GPIO          |
//! |                | ActuatorPort       | ESP32 PWM, GPIO          |
//! | `log_sink`     | EventSink          | Serial log output        |
//! | `nvs`          | ConfigPort         | NVS / in-memory store    |
//! |                | StoragePort        |                          |
//! | `rpc_adapter`  | EventSink          | FlatBuffers RPC transport|
//! | `time`         | TimePort           | ESP32 system timer       |
//! | `tls_transport`| Transport          | TCP + TLS 1.3 (PSK)     |
//! | `wifi`         | ConnectivityPort   | ESP-IDF WiFi STA         |
//! |                | EventSink          | Network event forwarding |

pub mod ble;
pub mod hardware;
pub mod log_sink;
pub mod nvs;
pub mod rpc_adapter;
pub mod time;
pub mod tls_transport;
pub mod wifi;
pub mod mdns;
pub mod device_id;
pub(super) mod utils;
