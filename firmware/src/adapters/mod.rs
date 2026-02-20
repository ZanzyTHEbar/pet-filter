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
//! | `tls_transport`| Transport          | TCP + TLS 1.3 (PSK/X509)|
//! | `cert_store`   | CertStore          | X.509 cert flash store  |
//! | `wifi`         | ConnectivityPort   | ESP-IDF WiFi STA         |
//! |                | EventSink          | Network event forwarding |

pub mod ble;
pub mod ble_transport;
pub mod cert_store;
pub mod device_id;
pub mod hardware;
pub mod log_sink;
pub mod mdns;
pub mod nvs;
pub mod rpc_adapter;
pub mod time;
pub mod tls_transport;
pub(super) mod utils;
pub mod wifi;
