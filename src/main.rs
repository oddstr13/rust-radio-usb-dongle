#![deny(unused_must_use)]
#![no_main]
#![no_std]

use core::str;
use core::sync::atomic::{self, Ordering};



use cortex_m_rt::entry;
use hal::clocks::{self, Clocks};
use hal::ieee802154::{self, Channel, Packet, TxPower};
use hal::pac::ficr;

use core::panic::PanicInfo;
use cortex_m::asm;

// USB Serial
use hal::usbd::{UsbPeripheral, Usbd};
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
use usbd_serial::{SerialPort, USB_CLASS_CDC};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{}", info);

    // abort instruction: triggers a HardFault exception which causes probe-run to exit
    asm::udf()
}

#[entry]
fn main() -> ! {
    static mut CLOCKS: Option<
        Clocks<clocks::ExternalOscillator, clocks::ExternalOscillator, clocks::LfOscStarted>,
    > = None;
    
    let periph = hal::pac::Peripherals::take().unwrap();
    let clocks = Clocks::new(periph.CLOCK);
    let clocks = clocks.enable_ext_hfosc();
    let clocks = clocks.set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass);
    let clocks = clocks.start_lfclk();
    let _clocks = clocks.enable_ext_hfosc();
    //let board = hal::init().unwrap();
    let clocks = unsafe { CLOCKS.get_or_insert(_clocks) };


    let usb_bus = Usbd::new(UsbPeripheral::new(periph.USBD, &clocks));
    let mut serial = SerialPort::new(&usb_bus);


    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("https://OpenShell.no")
        .product("radio-usb-dongle")
        .serial_number("TEST") // TODO: Use device ID
        .device_class(USB_CLASS_CDC)
        .max_packet_size_0(64) // (makes control transfers 8x faster)
        .build();
    

    let mut radio = hal::ieee802154::Radio::init(periph.RADIO, clocks);

    // these are the default settings of the DK's radio
    // NOTE if you ran `change-channel` then you may need to update the channel here
    radio.set_channel(Channel::_20); // <- must match the Dongle's listening channel
    radio.set_txpower(TxPower::Pos8dBm);

    let mut packet = Packet::new();

    // these three are equivalent
    // let msg: &[u8; 5] = &[72, 101, 108, 108, 111];
    // let msg: &[u8; 5] = &[b'H', b'e', b'l', b'l', b'o'];
    let mut _msg = [b'H', b'e', b'l', b'l', b'o', b' ', b' '];
    let msg: &mut[u8] = &mut _msg[..];

    log::info!(
        "sending: {}",
        str::from_utf8(msg).expect("msg is not valid UTF-8 data")
    );

    for n in 0..=9 {

        msg[6] = n + 48;

        packet.copy_from_slice(msg);
        //radio.send(&mut packet);
    }

    // Turn off TX for the love of the spectrum!
    radio.energy_detection_scan(1);

    let mut receiving = false;

    loop {
        if !receiving {
            radio.recv_async_start(&mut packet);
            receiving = true;
        } else if radio.recv_async_poll() {
            let res = radio.recv_async_sync();
            receiving = false;
            match res {
                Ok(_crc) => {
                    serial.write(b"Received: ").unwrap();
                    serial.write(str::from_utf8(&*packet).expect("Data not UTF-8").as_bytes()).unwrap();
                    serial.write(b"\r\n").unwrap();
                    packet.copy_from_slice(b"ACK");
                    radio.send(&mut packet);
                    radio.energy_detection_scan(1);
                },
                Err(_) => {
                    serial.write(b"RX failed\r\n").unwrap();
                },
            }
        }

        if usb_dev.poll(&mut [&mut serial]) {
            let mut buf = [0u8; 64];

            match serial.read(&mut buf) {
                Ok(count) if count > 0 => {
                    // Echo back in upper case
                    for c in buf[0..count].iter_mut() {
                        if 0x61 <= *c && *c <= 0x7a {
                            *c &= !0x20;
                        }
                        // Stop on receiving Q
                        if c == &b'Q' {
                            // force any pending memory operation to complete before the BKPT instruction that follows
                            atomic::compiler_fence(Ordering::SeqCst);
                            asm::bkpt()
                        }
                    }

                    let mut write_offset = 0;
                    while write_offset < count {
                        match serial.write(&buf[write_offset..count]) {
                            Ok(len) if len > 0 => {
                                write_offset += len;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

    }
}
