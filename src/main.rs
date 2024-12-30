use core::str;
use std::{
    net::Ipv4Addr,
    str::FromStr,
    sync::{Arc, Mutex},
};

use anyhow::{Ok, Result};
use chrono::{NaiveTime, TimeDelta, Timelike, Utc};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        delay::Delay,
        gpio::{Gpio25, Gpio26, Gpio32, Gpio33, InputOutput, InputPin, OutputPin, Pin, PinDriver},
        peripheral::{self},
        prelude::Peripherals,
        task::block_on,
    },
    http::{
        server::{Configuration, EspHttpServer},
        Method,
    },
    io::{EspIOError, Write},
    ipv4,
    netif::{EspNetif, NetifConfiguration, NetifStack},
    nvs::EspDefaultNvsPartition,
    sntp::{EspSntp, SyncStatus},
    wifi::{
        BlockingWifi, ClientConfiguration, Configuration as WifiConfiguration, EspWifi, WifiDriver,
    },
};
use log::info;
use serde_json::{json, Value};

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;

    // Connect to the Wi-Fi network
    let _wifi = wifi("Wifi1", "pass1", peripherals.modem, sysloop)?;

    // Set the HTTP server
    let mut server = EspHttpServer::new(&Configuration::default())?;

    let led = Arc::new(Mutex::new(PinDriver::output(peripherals.pins.gpio2)?));
    led.lock().unwrap().set_high()?;

    // let aspersores = Aspersores1::new_default(
    //     peripherals.pins.gpio32,
    //     peripherals.pins.gpio33,
    //     peripherals.pins.gpio25,
    //     peripherals.pins.gpio26,
    // );
    let aspersores = Aspersores2::new_default(
        peripherals.pins.gpio32,
        peripherals.pins.gpio33,
        peripherals.pins.gpio25,
    );

    aspersores.register_http_handlers(&mut server);

    println!("Server awaiting connection");

    // Network Time Protocol
    let ntp = EspSntp::new_default().unwrap();
    while ntp.get_sync_status() != SyncStatus::Completed {}

    block_on(async {
        loop {
            {
                led.lock().unwrap().toggle().unwrap();
            }

            let delay = Delay::new_default();
            delay.delay_ms(1000);

            aspersores.day_execution();
        }
    })
}

pub fn wifi(
    ssid: &str,
    pass: &str,
    modem: impl peripheral::Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
) -> Result<Box<EspWifi<'static>>> {
    let nvs = EspDefaultNvsPartition::take()?;
    let wifi_driver = WifiDriver::new(modem, sysloop.clone(), Some(nvs))?;
    let netmask = u8::from_str("24")?;
    let conf = NetifConfiguration {
        ip_configuration: ipv4::Configuration::Client(ipv4::ClientConfiguration::Fixed(
            ipv4::ClientSettings {
                ip: Ipv4Addr::from_str("192.168.1.149")?, // ########## IP ##########
                subnet: ipv4::Subnet {
                    gateway: Ipv4Addr::from_str("192.168.1.1")?,
                    mask: ipv4::Mask(netmask),
                },
                // Can also be set to Ipv4Addrs if you need DNS
                dns: Some(Ipv4Addr::new(192, 168, 1, 1)),
                secondary_dns: None,
            },
        )),
        ..NetifConfiguration::wifi_default_client()
    };

    let mut esp_wifi = EspWifi::wrap_all(
        wifi_driver,
        EspNetif::new_with_conf(&NetifConfiguration { ..conf })?,
        EspNetif::new(NetifStack::Ap)?,
    )?;

    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    wifi.set_configuration(&WifiConfiguration::Client(ClientConfiguration {
        ssid: ssid
            .try_into()
            .expect("Could not parse the given SSID into WiFi config"),
        password: pass
            .try_into()
            .expect("Could not parse the given password into WiFi config"),
        ..Default::default()
    }))?;

    info!("Starting wifi...");

    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

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
    pub fn new(name: String, pin: T, duration: TimeDelta, init_time: NaiveTime) -> Self {
        Aspersor {
            name,
            pin: Arc::new(Mutex::new(PinDriver::input_output(pin).unwrap())),
            init_time: Arc::new(Mutex::new(init_time.num_seconds_from_midnight())),
            duration: Arc::new(Mutex::new(duration.num_seconds().try_into().unwrap())),
        }
    }

    pub fn start(&self) {
        let current_time = Utc::now()
            .checked_sub_signed(TimeDelta::hours(3))
            .unwrap()
            .time()
            .num_seconds_from_midnight();
        let init_time = *self.init_time.lock().unwrap();
        let duration = *self.duration.lock().unwrap();

        if current_time >= init_time {
            // We wait only the amount of time remaining from the original duration
            let time_left = (duration + init_time).saturating_sub(current_time);

            if time_left > 0 {
                self.pin.lock().unwrap().set_high().unwrap();

                // Sleep for the pin duration
                let delay = Delay::new_default();
                delay.delay_ms(time_left * 1000);

                self.pin.lock().unwrap().set_low().unwrap();
            }
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

    pub fn update_duration_and_init_time(&self, server: &mut EspHttpServer<'a>) {
        let duration = self.duration.clone();
        let init_time = self.init_time.clone();

        unsafe {
            server
                .fn_handler_nonstatic(
                    &format!("/update_aspersor/{}", self.name),
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let uri = request.uri();
                        let received_duration = parse_http_uri(uri, "duration");
                        let received_init_time = parse_http_uri(uri, "init_time");

                        println!(
                            "duration: {} - init_time: {}",
                            received_duration, received_init_time
                        );

                        *duration.lock().unwrap() = received_duration.parse().unwrap();
                        *init_time.lock().unwrap() = received_init_time.parse().unwrap();

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
    pub fn new_default(gpio32: Gpio32, gpio33: Gpio33, gpio25: Gpio25) -> Self {
        Aspersores2 {
            toberas_afuera: Aspersor::new(
                "toberas_afuera".to_string(),
                gpio32,
                TimeDelta::minutes(45),
                NaiveTime::from_hms_milli_opt(6, 15, 0, 0).unwrap(),
            ),
            rotor_frente: Aspersor::new(
                "rotor_frente".to_string(),
                gpio33,
                TimeDelta::minutes(40),
                NaiveTime::from_hms_milli_opt(7, 0, 0, 0).unwrap(),
            ),
            costado_180: Aspersor::new(
                "costado_180".to_string(),
                gpio25,
                TimeDelta::hours(1)
                    .checked_add(&TimeDelta::minutes(15))
                    .unwrap(),
                NaiveTime::from_hms_milli_opt(5, 0, 0, 0).unwrap(),
            ),
            manual_mode: Arc::new(Mutex::new(false)),
        }
    }

    pub fn day_execution(&self) {
        let is_manual_mode = *self.manual_mode.lock().unwrap();
        if !is_manual_mode {
            self.costado_180.start();
            self.toberas_afuera.start();
            self.rotor_frente.start();
        }
    }

    pub fn register_http_handlers(&self, server: &mut EspHttpServer<'a>) {
        self.costado_180.toggle_pin(server);
        self.toberas_afuera.toggle_pin(server);
        self.rotor_frente.toggle_pin(server);

        self.costado_180.update_duration_and_init_time(server);
        self.toberas_afuera.update_duration_and_init_time(server);
        self.rotor_frente.update_duration_and_init_time(server);

        unsafe {
            let manual_mode = self.manual_mode.clone();

            server
                .fn_handler_nonstatic(
                    "/toggle/manual_mode",
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let mut manual_mode = manual_mode.lock().unwrap();
                        *manual_mode = !(*manual_mode);

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

/// REMAINDER!: Remember to update the sta IP when changing chip
/// TODO!: read ip from env file?
/// **Not recomended pins: 6 - 11, 16 - 17
struct Aspersores1<'a> {
    microaspersores_frente: Aspersor<'a, Gpio32>,
    goteros: Aspersor<'a, Gpio33>,
    atras_360: Aspersor<'a, Gpio25>,
    atras_pileta: Aspersor<'a, Gpio26>,
    manual_mode: Arc<Mutex<bool>>,
}

impl<'a> Aspersores1<'a> {
    pub fn new_default(gpio32: Gpio32, gpio33: Gpio33, gpio25: Gpio25, gpio26: Gpio26) -> Self {
        Aspersores1 {
            microaspersores_frente: Aspersor::new(
                "microaspersores_frente".to_string(),
                gpio32,
                TimeDelta::minutes(20),
                NaiveTime::from_hms_milli_opt(22, 0, 0, 0).unwrap(),
            ),
            goteros: Aspersor::new(
                "goteros".to_string(),
                gpio33,
                TimeDelta::hours(5),
                NaiveTime::from_hms_milli_opt(16, 0, 0, 0).unwrap(),
            ),
            atras_360: Aspersor::new(
                "atras_360".to_string(),
                gpio25,
                TimeDelta::hours(1)
                    .checked_add(&TimeDelta::minutes(30))
                    .unwrap(),
                NaiveTime::from_hms_milli_opt(3, 30, 0, 0).unwrap(),
            ),
            atras_pileta: Aspersor::new(
                "atras_pileta".to_string(),
                gpio26,
                TimeDelta::hours(1),
                NaiveTime::from_hms_milli_opt(21, 0, 0, 0).unwrap(),
            ),
            manual_mode: Arc::new(Mutex::new(false)),
        }
    }

    pub fn day_execution(&self) {
        let is_manual_mode = *self.manual_mode.lock().unwrap();
        if !is_manual_mode {
            self.microaspersores_frente.start();
            self.goteros.start();
            self.atras_360.start();
            self.atras_pileta.start();
        }
    }

    pub fn register_http_handlers(&self, server: &mut EspHttpServer<'a>) {
        self.microaspersores_frente.toggle_pin(server);
        self.goteros.toggle_pin(server);
        self.atras_360.toggle_pin(server);
        self.atras_pileta.toggle_pin(server);

        self.microaspersores_frente
            .update_duration_and_init_time(server);
        self.goteros.update_duration_and_init_time(server);
        self.atras_360.update_duration_and_init_time(server);
        self.atras_pileta.update_duration_and_init_time(server);

        unsafe {
            let manual_mode = self.manual_mode.clone();

            server
                .fn_handler_nonstatic(
                    "/toggle/manual_mode",
                    Method::Get,
                    move |request| -> core::result::Result<(), EspIOError> {
                        let mut manual_mode = manual_mode.lock().unwrap();
                        *manual_mode = !(*manual_mode);

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

            let manual_mode = self.manual_mode.clone();
            let microaspersores_frente = self.microaspersores_frente.clone();
            let goteros = self.goteros.clone();
            let atras_360 = self.atras_360.clone();
            let atras_pileta = self.atras_pileta.clone();

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
                                microaspersores_frente.to_json(),
                                goteros.to_json(),
                                atras_360.to_json(),
                                atras_pileta.to_json(),
                            ]
                        });
                        let data = json.to_string();
                        let data = data.as_bytes();
                        response.write_all(data)?;

                        core::result::Result::Ok(())
                    },
                )
                .unwrap();
        }
    }
}
