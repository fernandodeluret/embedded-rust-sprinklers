use core::str;
use std::{
    net::Ipv4Addr,
    str::FromStr,
    sync::{Arc, Mutex},
};

use anyhow::{Ok, Result};
use chrono::{TimeDelta, Timelike, Utc};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        delay::Delay,
        gpio::{Gpio25, Gpio26, Gpio32, Gpio33, InputOutput, InputPin, OutputPin, Pin, PinDriver},
        peripheral::{self},
        prelude::Peripherals,
    },
    http::{
        server::{Configuration, EspHttpServer},
        Method,
    },
    io::{EspIOError, Write},
    ipv4,
    netif::{EspNetif, NetifConfiguration, NetifStack},
    nvs::{EspDefaultNvsPartition, EspNvs, EspNvsPartition, NvsDefault},
    wifi::{
        AccessPointConfiguration, AuthMethod, Configuration as WifiConfiguration, EspWifi,
        WifiDriver,
    },
};
use log::info;
use serde_json::{json, Value};

mod root_html;

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;

    // NVS (Non-Volatile Storage) is ESP's key-value flash storage system
    let nvs_partition = EspDefaultNvsPartition::take()?;

    // Create NVS namespace for our time data
    let nvs = EspNvs::new(nvs_partition.clone(), "sprinklers", true)?;

    // Read saved time offset from NVS (or default to 0)
    let saved_offset: i64 = nvs.get_i64("time_offset")?.unwrap_or(0);
    info!("Loaded time offset from NVS: {}", saved_offset);

    // Shared time offset (Arc<Mutex> so handlers can access it)
    let time_offset: Arc<Mutex<i64>> = Arc::new(Mutex::new(saved_offset));
    let nvs = Arc::new(Mutex::new(nvs));

    // Create the Wi-Fi network
    // REMEMBER: to update the Network name for Aspersores1 or Aspersores2
    let _wifi = wifi(
        "WifiSprinklersFront", //  WifiSprinklersFront / WifiSprinklersBack
        "pass00123",
        peripherals.modem,
        sysloop,
        nvs_partition,
    )?;

    // Set the HTTP server
    let mut server = EspHttpServer::new(&Configuration::default())?;

    let led = Arc::new(Mutex::new(PinDriver::output(peripherals.pins.gpio2)?));
    led.lock().unwrap().set_high()?;

    // let aspersores = Aspersores1::new_with_nvs(
    //     peripherals.pins.gpio32,
    //     peripherals.pins.gpio33,
    //     peripherals.pins.gpio25,
    //     peripherals.pins.gpio26,
    //     &nvs.lock().unwrap(), // Pass NVS reference for loading
    // );

    let aspersores = Aspersores2::new_with_nvs(
        peripherals.pins.gpio32,
        peripherals.pins.gpio33,
        peripherals.pins.gpio25,
        &nvs.lock().unwrap(), // Pass NVS reference for loading
    );

    aspersores.register_http_handlers(&mut server, time_offset.clone(), nvs.clone());

    println!("Server awaiting connection at http://192.168.1.1");

    // We keep track of the time here for potential reboot
    let mut last_nvs_save: i64 = 0;

    loop {
        {
            led.lock().unwrap().toggle().unwrap();
        }

        // Get current time offset
        let offset = *time_offset.lock().unwrap();
        let real_time = Utc::now().timestamp() + offset;

        // Save absolute timestamp to NVS every 60 seconds
        if real_time - last_nvs_save >= 60 {
            let nvs_guard = nvs.lock().unwrap();
            nvs_guard.set_i64("time_offset", real_time).ok();
            last_nvs_save = real_time;
            info!("Saved time to NVS: {}", real_time);
        }

        // Non-blocking update - just checks time and toggles if needed
        aspersores.update_all(offset);

        // Short delay, doesn't block HTTP
        let delay = Delay::new_default();
        delay.delay_ms(1000);
    }
}

pub fn wifi(
    ssid: &str,
    pass: &str,
    modem: impl peripheral::Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
    nvs_partition: EspNvsPartition<NvsDefault>,
) -> Result<Box<EspWifi<'static>>> {
    // let nvs = EspDefaultNvsPartition::take()?;
    let wifi_driver = WifiDriver::new(modem, sysloop.clone(), Some(nvs_partition))?;

    // let netmask = u8::from_str("24")?;
    // let conf = NetifConfiguration {
    //     ip_configuration: Some(ipv4::Configuration::Client(
    //         ipv4::ClientConfiguration::Fixed(ipv4::ClientSettings {
    //             ip: Ipv4Addr::from_str("192.168.1.149")?, // ########## IP ##########
    //             subnet: ipv4::Subnet {
    //                 gateway: Ipv4Addr::from_str("192.168.1.1")?,
    //                 mask: ipv4::Mask(netmask),
    //             },
    //             // Can also be set to Ipv4Addrs if you need DNS
    //             dns: Some(Ipv4Addr::new(192, 168, 1, 1)),
    //             secondary_dns: None,
    //         }),
    //     )),
    //     ..NetifConfiguration::wifi_default_client()
    // };

    // let mut esp_wifi = EspWifi::wrap_all(
    //     wifi_driver,
    //     EspNetif::new_with_conf(&NetifConfiguration { ..conf })?,
    //     EspNetif::new(NetifStack::Ap)?,
    // )?;

    // let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    // wifi.set_configuration(&WifiConfiguration::Client(ClientConfiguration {
    //     ssid: ssid
    //         .try_into()
    //         .expect("Could not parse the given SSID into WiFi config"),
    //     password: pass
    //         .try_into()
    //         .expect("Could not parse the given password into WiFi config"),
    //     ..Default::default()
    // }))?;

    // info!("Starting wifi...");

    // wifi.start()?;
    // wifi.connect()?;
    // wifi.wait_netif_up()?;

    // let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    // info!("Wifi DHCP info: {:?}", ip_info);

    // #######################################################################

    let netmask = u8::from_str("24")?;

    let ap_netif_config = NetifConfiguration {
        ip_configuration: Some(ipv4::Configuration::Router(ipv4::RouterConfiguration {
            subnet: ipv4::Subnet {
                gateway: Ipv4Addr::from_str("192.168.1.1")?, // IP
                mask: ipv4::Mask(netmask),
            },
            dhcp_enabled: true, // ESP32 will assign IPs to connecting clients
            dns: Some(Ipv4Addr::from_str("192.168.1.1")?), // Optional: ESP32 as DNS server
            secondary_dns: None,
        })),
        ..NetifConfiguration::wifi_default_router()
    };

    let mut esp_wifi = EspWifi::wrap_all(
        wifi_driver,
        EspNetif::new(NetifStack::Sta)?, // STA interface (unused but required)
        EspNetif::new_with_conf(&ap_netif_config)?, // AP interface with custom IP
    )?;

    esp_wifi.set_configuration(&WifiConfiguration::AccessPoint(AccessPointConfiguration {
        ssid: ssid.try_into().expect("SSID too long"),
        password: pass.try_into().expect("Password too long"),
        auth_method: AuthMethod::WPA2Personal,
        ssid_hidden: false,
        channel: 1,
        max_connections: 4,
        ..Default::default()
    }))?;

    esp_wifi.start()?;

    let ip_info = esp_wifi.ap_netif().get_ip_info()?;
    info!("AP started at http://{}", ip_info.ip);

    Ok(Box::new(esp_wifi))
}

struct Aspersor<'a, T: Pin> {
    name: String,
    pin: Arc<Mutex<PinDriver<'a, T, InputOutput>>>,
    /// Time in seconds from midnight to start the pin
    init_time: Arc<Mutex<u32>>,
    /// Duration in seconds for the pin to be set as high
    duration: Arc<Mutex<u32>>,
}

impl<'a, T: Pin> Clone for Aspersor<'a, T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            pin: self.pin.clone(),
            init_time: self.init_time.clone(),
            duration: self.duration.clone(),
        }
    }
}

impl<'a, T: Pin + OutputPin + InputPin> Aspersor<'a, T> {
    // pub fn start(&self) {
    //     let current_time = Utc::now()
    //         .checked_sub_signed(TimeDelta::hours(3))
    //         .unwrap()
    //         .time()
    //         .num_seconds_from_midnight();
    //     let init_time = *self.init_time.lock().unwrap();
    //     let duration = *self.duration.lock().unwrap();

    //     if current_time >= init_time {
    //         // We wait only the amount of time remaining from the original duration
    //         let time_left = (duration + init_time).saturating_sub(current_time);

    //         if time_left > 0 {
    //             self.pin.lock().unwrap().set_high().unwrap();

    //             // Sleep for the pin duration
    //             let delay = Delay::new_default();
    //             delay.delay_ms(time_left * 1000);

    //             self.pin.lock().unwrap().set_low().unwrap();
    //         }
    //     }
    // }

    pub fn new_with_settings(name: String, pin: T, duration: u32, init_time: u32) -> Self {
        Aspersor {
            name,
            pin: Arc::new(Mutex::new(PinDriver::input_output(pin).unwrap())),
            init_time: Arc::new(Mutex::new(init_time)),
            duration: Arc::new(Mutex::new(duration)),
        }
    }

    /// Non-blocking: Call this frequently. It checks time and toggles pin.
    pub fn update(&self, current_time_secs: u32) {
        let init_time = *self.init_time.lock().unwrap();
        let duration = *self.duration.lock().unwrap();
        let end_time = init_time + duration;

        let mut pin = self.pin.lock().unwrap();

        // Should be ON if current time is within the schedule window
        let should_be_on = current_time_secs >= init_time && current_time_secs < end_time;

        if should_be_on && pin.is_low() {
            pin.set_high().ok();
            info!("{} turned ON", self.name);
        } else if !should_be_on && pin.is_high() {
            pin.set_low().ok();
            info!("{} turned OFF", self.name);
        }
    }

    pub fn toggle_pin(&self, server: &mut EspHttpServer<'a>) {
        let pin = self.pin.clone();

        unsafe {
            server
                .fn_handler_nonstatic(
                    &format!("/toggle/{}", self.name),
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        pin.lock().unwrap().toggle().unwrap();

                        let mut response = request.into_response(
                            200,
                            Some("OK"),
                            &[("Access-Control-Allow-Origin", "*")],
                        )?;
                        let json = json!({
                            "ok": true,
                        })
                        .to_string();
                        let data = json.as_bytes();
                        response.write_all(data)?;
                        core::result::Result::Ok(())
                    },
                )
                .unwrap();
        }
    }

    pub fn update_duration_and_init_time(
        &self,
        server: &mut EspHttpServer<'a>,
        nvs: Arc<Mutex<EspNvs<NvsDefault>>>,
    ) {
        let duration = self.duration.clone();
        let init_time = self.init_time.clone();
        let name = self.name.clone();

        // Create short NVS key (max 15 chars)
        // Use first 10 chars of name + suffix
        let nvs_key_base: String = name.chars().take(10).collect();

        unsafe {
            let nvs_key = nvs_key_base.clone();

            server
                .fn_handler_nonstatic(
                    &format!("/update_aspersor/{}", self.name),
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let uri = request.uri();
                        let received_duration: u32 =
                            parse_http_uri(uri, "duration").parse().unwrap_or(0);
                        let received_init_time: u32 =
                            parse_http_uri(uri, "init_time").parse().unwrap_or(0);

                        println!(
                            "Updating {}: duration={}, init_time={}",
                            name, received_duration, received_init_time
                        );

                        // Update in memory
                        *duration.lock().unwrap() = received_duration;
                        *init_time.lock().unwrap() = received_init_time;

                        // Save to NVS with short keys (max 15 chars)
                        {
                            let nvs = nvs.lock().unwrap();
                            let duration_key = format!("{}_d", nvs_key); // e.g., "toberas_af_d" (12 chars)
                            let init_key = format!("{}_i", nvs_key); // e.g., "toberas_af_i" (12 chars)

                            if let Err(e) = nvs.set_u32(&duration_key, received_duration) {
                                println!("NVS save error for {}: {:?}", duration_key, e);
                            }
                            if let Err(e) = nvs.set_u32(&init_key, received_init_time) {
                                println!("NVS save error for {}: {:?}", init_key, e);
                            }
                        }

                        let mut response = request.into_response(
                            200,
                            Some("OK"),
                            &[("Access-Control-Allow-Origin", "*")],
                        )?;
                        let json = json!({ "ok": true }).to_string();
                        response.write_all(json.as_bytes())?;
                        core::result::Result::Ok(())
                    },
                )
                .unwrap();
        }
    }

    pub fn to_json(&self) -> Value {
        let pin = self.pin.lock().unwrap();
        json!({
            "name": self.name,
            "pin": pin.pin(),
            "on": pin.is_high(),
            "init_time": *self.init_time.lock().unwrap(),
            "duration": *self.duration.lock().unwrap(),
        })
    }
}

struct Aspersores2<'a> {
    toberas_afuera: Aspersor<'a, Gpio32>,
    rotor_frente: Aspersor<'a, Gpio33>,
    costado_180: Aspersor<'a, Gpio25>,
    manual_mode: Arc<Mutex<bool>>,
}

impl<'a> Aspersores2<'a> {
    pub fn new_with_nvs(
        gpio32: Gpio32,
        gpio33: Gpio33,
        gpio25: Gpio25,
        nvs: &EspNvs<NvsDefault>,
    ) -> Self {
        // Default values (in seconds)
        let (toberas_init, toberas_dur) = load_aspersor_settings(
            nvs,
            "toberas_afuera",
            6 * 3600 + 15 * 60, // 6:15 AM
            45 * 60,            // 45 minutes
        );

        let (rotor_init, rotor_dur) = load_aspersor_settings(
            nvs,
            "rotor_frente",
            7 * 3600, // 7:00 AM
            40 * 60,  // 40 minutes
        );

        let (costado_init, costado_dur) = load_aspersor_settings(
            nvs,
            "costado_180",
            5 * 3600, // 5:00 AM
            75 * 60,  // 1h 15m
        );

        // Load manual_mode from NVS (default to false)
        let saved_manual_mode = nvs.get_u8("manual_mode").ok().flatten().unwrap_or(0) != 0;
        info!("Loaded manual_mode: {}", saved_manual_mode);

        Aspersores2 {
            toberas_afuera: Aspersor::new_with_settings(
                "toberas_afuera".to_string(),
                gpio32,
                toberas_dur,
                toberas_init,
            ),
            rotor_frente: Aspersor::new_with_settings(
                "rotor_frente".to_string(),
                gpio33,
                rotor_dur,
                rotor_init,
            ),
            costado_180: Aspersor::new_with_settings(
                "costado_180".to_string(),
                gpio25,
                costado_dur,
                costado_init,
            ),
            manual_mode: Arc::new(Mutex::new(saved_manual_mode)), // Use loaded value
        }
    }

    // pub fn day_execution(&self) {
    //     let is_manual_mode = *self.manual_mode.lock().unwrap();
    //     if !is_manual_mode {
    //         self.costado_180.start();
    //         self.toberas_afuera.start();
    //         self.rotor_frente.start();
    //     }
    // }

    /// Non-blocking: Call this every loop iteration
    pub fn update_all(&self, time_offset: i64) {
        let is_manual_mode = *self.manual_mode.lock().unwrap();
        if is_manual_mode {
            return; // In manual mode, don't auto-control
        }

        // Get current time in seconds from midnight (UTC-3)
        let adjusted = Utc::now() + TimeDelta::seconds(time_offset);
        let tz = chrono::FixedOffset::west_opt(3 * 3600).unwrap();
        let current_time = adjusted
            .with_timezone(&tz)
            .time()
            .num_seconds_from_midnight();

        self.costado_180.update(current_time);
        self.toberas_afuera.update(current_time);
        self.rotor_frente.update(current_time);
    }

    pub fn register_http_handlers(
        &self,
        server: &mut EspHttpServer<'a>,
        time_offset: Arc<Mutex<i64>>,
        nvs: Arc<Mutex<EspNvs<NvsDefault>>>,
    ) {
        self.costado_180.toggle_pin(server);
        self.toberas_afuera.toggle_pin(server);
        self.rotor_frente.toggle_pin(server);

        self.costado_180
            .update_duration_and_init_time(server, nvs.clone());
        self.toberas_afuera
            .update_duration_and_init_time(server, nvs.clone());
        self.rotor_frente
            .update_duration_and_init_time(server, nvs.clone());

        unsafe {
            let manual_mode = self.manual_mode.clone();
            let nvs_for_manual = nvs.clone();

            server
                .fn_handler_nonstatic(
                    "/toggle/manual_mode",
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let mut manual_mode = manual_mode.lock().unwrap();
                        *manual_mode = !(*manual_mode);

                        // Save to NVS
                        {
                            let nvs = nvs_for_manual.lock().unwrap();
                            let value: u8 = if *manual_mode { 1 } else { 0 };
                            if let Err(e) = nvs.set_u8("manual_mode", value) {
                                println!("Failed to save manual_mode: {:?}", e);
                            }
                        }

                        let mut response = request.into_response(
                            200,
                            Some("OK"),
                            &[("Access-Control-Allow-Origin", "*")],
                        )?;
                        let json = json!({
                            "ok": true,
                        })
                        .to_string();
                        response.write_all(json.as_bytes())?;

                        core::result::Result::Ok(())
                    },
                )
                .unwrap();

            let manual_mode = self.manual_mode.clone();
            let costado_180 = self.costado_180.clone();
            let toberas_afuera = self.toberas_afuera.clone();
            let rotor_frente = self.rotor_frente.clone();

            server
                .fn_handler_nonstatic(
                    "/get_info",
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let mut response = request.into_response(200, Some("OK"), &[
                            ("Access-Control-Allow-Origin", "*")
                        ])?;

                        let json = json!({
                            "time": format!("{}", Utc::now().checked_sub_signed(TimeDelta::hours(3)).unwrap()),
                            "manual_mode": *manual_mode.lock().unwrap(),
                            "aspersores": [
                                costado_180.to_json(),
                                toberas_afuera.to_json(),
                                rotor_frente.to_json(),
                            ]
                        });
                        let data = json.to_string();
                        let data = data.as_bytes();
                        response.write_all(data)?;

                        core::result::Result::Ok(())
                    },
                )
                .unwrap();

            let time_offset_for_sync = time_offset.clone();

            server
                .fn_handler_nonstatic(
                    "/",
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let mut response = request.into_response(
                            200,
                            Some("OK"),
                            &[
                                ("Content-Type", "text/html; charset=utf-8"),
                                ("Access-Control-Allow-Origin", "*"),
                            ],
                        )?;

                        // Apply the saved time offset!
                        let offset = *time_offset_for_sync.lock().unwrap();
                        let adjusted_time = Utc::now() + TimeDelta::seconds(offset);
                        let tz = chrono::FixedOffset::west_opt(3 * 3600).unwrap();
                        let server_time = format!("{}", adjusted_time.with_timezone(&tz));

                        let html = root_html::get_root_html(&server_time);
                        response.write_all(html.as_bytes())?;

                        core::result::Result::Ok(())
                    },
                )
                .unwrap();

            // Clone the Arc's for the handler
            let time_offset_clone = time_offset.clone();
            let nvs_clone = nvs.clone();

            server
                .fn_handler_nonstatic(
                    "/set_time",
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let uri = request.uri();
                        let timestamp_str = parse_http_uri(uri, "timestamp");
                        let client_timestamp: i64 = timestamp_str.parse().unwrap_or(0);

                        // Calculate offset: client_time - our_boot_time
                        let our_time = Utc::now().timestamp();
                        let offset = client_timestamp - our_time;

                        // Save to memory
                        *time_offset_clone.lock().unwrap() = offset;

                        // Save to NVS (persists across reboots!)
                        {
                            let nvs = nvs_clone.lock().unwrap();
                            nvs.set_i64("time_offset", offset).ok();
                        }

                        info!("Time synced! Offset: {} seconds", offset);

                        let mut response = request.into_response(
                            200,
                            Some("OK"),
                            &[("Access-Control-Allow-Origin", "*")],
                        )?;

                        let json = json!({ "ok": true });
                        response.write_all(json.to_string().as_bytes())?;
                        core::result::Result::Ok(())
                    },
                )
                .unwrap();
        }
    }
}

fn parse_http_uri<'a>(uri: &'a str, param: &str) -> &'a str {
    //TODO!: start reading from the '?' to avoid conflicts with route having the same sub-string
    let i = uri.find(param).unwrap();
    let initial_i = i + param.len() + 1;
    let partial_uri = &uri[initial_i..];
    let final_i = initial_i + partial_uri.find("&").unwrap_or(partial_uri.len());

    &uri[initial_i..final_i]
}

/// **Not recomended pins: 6 - 11, 16 - 17
struct Aspersores1<'a> {
    microaspersores_frente: Aspersor<'a, Gpio32>,
    goteros: Aspersor<'a, Gpio33>,
    atras_360: Aspersor<'a, Gpio25>,
    atras_pileta: Aspersor<'a, Gpio26>,
    manual_mode: Arc<Mutex<bool>>,
}

impl<'a> Aspersores1<'a> {
    pub fn new_with_nvs(
        gpio32: Gpio32,
        gpio33: Gpio33,
        gpio25: Gpio25,
        gpio26: Gpio26,
        nvs: &EspNvs<NvsDefault>,
    ) -> Self {
        // Default values (in seconds)
        let (micro_init, micro_dur) = load_aspersor_settings(
            nvs,
            "micro_frente", // shortened to fit NVS 15-char key limit
            22 * 3600,      // 22:00
            20 * 60,        // 20 minutes
        );

        let (goteros_init, goteros_dur) = load_aspersor_settings(
            nvs,
            "goteros",
            16 * 3600, // 16:00
            5 * 3600,  // 5 hours
        );

        let (atras360_init, atras360_dur) = load_aspersor_settings(
            nvs,
            "atras_360",
            3 * 3600 + 30 * 60, // 3:30 AM
            90 * 60,            // 1h 30m
        );

        let (pileta_init, pileta_dur) = load_aspersor_settings(
            nvs,
            "atras_pileta",
            21 * 3600, // 21:00
            60 * 60,   // 1 hour
        );

        // Load manual_mode from NVS (default to false)
        let saved_manual_mode = nvs.get_u8("manual_mode").ok().flatten().unwrap_or(0) != 0;
        info!("Loaded manual_mode: {}", saved_manual_mode);

        Aspersores1 {
            microaspersores_frente: Aspersor::new_with_settings(
                "micro_frente".to_string(), // Use same short name as NVS key
                gpio32,
                micro_dur,
                micro_init,
            ),
            goteros: Aspersor::new_with_settings(
                "goteros".to_string(),
                gpio33,
                goteros_dur,
                goteros_init,
            ),
            atras_360: Aspersor::new_with_settings(
                "atras_360".to_string(),
                gpio25,
                atras360_dur,
                atras360_init,
            ),
            atras_pileta: Aspersor::new_with_settings(
                "atras_pileta".to_string(),
                gpio26,
                pileta_dur,
                pileta_init,
            ),
            manual_mode: Arc::new(Mutex::new(saved_manual_mode)), // Use loaded value
        }
    }

    /// Non-blocking: Call this every loop iteration
    pub fn update_all(&self, time_offset: i64) {
        let is_manual_mode = *self.manual_mode.lock().unwrap();
        if is_manual_mode {
            return; // In manual mode, don't auto-control
        }

        // Get current time in seconds from midnight (UTC-3)
        let adjusted = Utc::now() + TimeDelta::seconds(time_offset);
        let tz = chrono::FixedOffset::west_opt(3 * 3600).unwrap();
        let current_time = adjusted
            .with_timezone(&tz)
            .time()
            .num_seconds_from_midnight();

        self.atras_360.update(current_time);
        self.atras_pileta.update(current_time);
        self.microaspersores_frente.update(current_time);
        self.goteros.update(current_time);
    }

    pub fn register_http_handlers(
        &self,
        server: &mut EspHttpServer<'a>,
        time_offset: Arc<Mutex<i64>>, // Add this parameter
        nvs: Arc<Mutex<EspNvs<NvsDefault>>>,
    ) {
        self.microaspersores_frente.toggle_pin(server);
        self.goteros.toggle_pin(server);
        self.atras_360.toggle_pin(server);
        self.atras_pileta.toggle_pin(server);

        self.microaspersores_frente
            .update_duration_and_init_time(server, nvs.clone());
        self.goteros
            .update_duration_and_init_time(server, nvs.clone());
        self.atras_360
            .update_duration_and_init_time(server, nvs.clone());
        self.atras_pileta
            .update_duration_and_init_time(server, nvs.clone());

        unsafe {
            let manual_mode = self.manual_mode.clone();
            let nvs_for_manual = nvs.clone();

            server
                .fn_handler_nonstatic(
                    "/toggle/manual_mode",
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let mut manual_mode = manual_mode.lock().unwrap();
                        *manual_mode = !(*manual_mode);

                        // Save to NVS
                        {
                            let nvs = nvs_for_manual.lock().unwrap();
                            let value: u8 = if *manual_mode { 1 } else { 0 };
                            if let Err(e) = nvs.set_u8("manual_mode", value) {
                                println!("Failed to save manual_mode: {:?}", e);
                            }
                        }

                        let mut response = request.into_response(
                            200,
                            Some("OK"),
                            &[("Access-Control-Allow-Origin", "*")],
                        )?;
                        let json = json!({
                            "ok": true,
                        })
                        .to_string();
                        response.write_all(json.as_bytes())?;

                        core::result::Result::Ok(())
                    },
                )
                .unwrap();

            let manual_mode = self.manual_mode.clone();
            let microaspersores_frente = self.microaspersores_frente.clone();
            let goteros = self.goteros.clone();
            let atras_360 = self.atras_360.clone();
            let atras_pileta = self.atras_pileta.clone();
            let time_offset_for_info = time_offset.clone();

            server
                .fn_handler_nonstatic(
                    "/get_info",
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let mut response = request.into_response(
                            200,
                            Some("OK"),
                            &[("Access-Control-Allow-Origin", "*")],
                        )?;

                        // Use time offset for correct time display
                        let offset = *time_offset_for_info.lock().unwrap();
                        let adjusted_time = Utc::now() + TimeDelta::seconds(offset);
                        let tz = chrono::FixedOffset::west_opt(3 * 3600).unwrap();

                        let json = json!({
                            "time": format!("{}", adjusted_time.with_timezone(&tz)),
                            "manual_mode": *manual_mode.lock().unwrap(),
                            "aspersores": [
                                microaspersores_frente.to_json(),
                                goteros.to_json(),
                                atras_360.to_json(),
                                atras_pileta.to_json(),
                            ]
                        });
                        response.write_all(json.to_string().as_bytes())?;

                        core::result::Result::Ok(())
                    },
                )
                .unwrap();

            // /Root endpoint
            let time_offset_for_sync = time_offset.clone();

            server
                .fn_handler_nonstatic(
                    "/",
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let mut response = request.into_response(
                            200,
                            Some("OK"),
                            &[
                                ("Content-Type", "text/html; charset=utf-8"),
                                ("Access-Control-Allow-Origin", "*"),
                            ],
                        )?;

                        let offset = *time_offset_for_sync.lock().unwrap();
                        let adjusted_time = Utc::now() + TimeDelta::seconds(offset);
                        let tz = chrono::FixedOffset::west_opt(3 * 3600).unwrap();
                        let server_time = format!("{}", adjusted_time.with_timezone(&tz));

                        let html = root_html::get_root_html(&server_time);
                        response.write_all(html.as_bytes())?;

                        core::result::Result::Ok(())
                    },
                )
                .unwrap();

            // /set_time endpoint
            let time_offset_clone = time_offset.clone();
            let nvs_clone = nvs.clone();

            server
                .fn_handler_nonstatic(
                    "/set_time",
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let uri = request.uri();
                        let timestamp_str = parse_http_uri(uri, "timestamp");
                        let client_timestamp: i64 = timestamp_str.parse().unwrap_or(0);

                        let our_time = Utc::now().timestamp();
                        let offset = client_timestamp - our_time;

                        *time_offset_clone.lock().unwrap() = offset;

                        {
                            let nvs = nvs_clone.lock().unwrap();
                            nvs.set_i64("time_offset", offset).ok();
                        }

                        info!("Time synced! Offset: {} seconds", offset);

                        let mut response = request.into_response(
                            200,
                            Some("OK"),
                            &[("Access-Control-Allow-Origin", "*")],
                        )?;

                        let json = json!({ "ok": true });
                        response.write_all(json.to_string().as_bytes())?;
                        core::result::Result::Ok(())
                    },
                )
                .unwrap();
        }
    }
}

/// Helper function to load aspersor settings from NVS
fn load_aspersor_settings(
    nvs: &EspNvs<NvsDefault>,
    name: &str,
    default_init_time: u32,
    default_duration: u32,
) -> (u32, u32) {
    // Use first 10 chars for NVS key (must match save format!)
    let nvs_key: String = name.chars().take(10).collect();
    let duration_key = format!("{}_d", nvs_key);
    let init_key = format!("{}_i", nvs_key);

    let init_time = nvs
        .get_u32(&init_key)
        .ok()
        .flatten()
        .unwrap_or(default_init_time);
    let duration = nvs
        .get_u32(&duration_key)
        .ok()
        .flatten()
        .unwrap_or(default_duration);

    info!(
        "Loaded {}: init_time={}, duration={} (keys: {}, {})",
        name, init_time, duration, init_key, duration_key
    );
    (init_time, duration)
}
