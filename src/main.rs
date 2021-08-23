#![deny(unused_must_use)]
#![no_main]
#![no_std]

use core::str;
use core::sync::atomic::{self, Ordering};



use cortex_m_rt::entry;
use hal::clocks::{self, Clocks};
use hal::ieee802154::{self, Channel, Packet, TxPower};

use core::panic::PanicInfo;
use cortex_m::asm;

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
    
    if let Some(periph) = hal::pac::Peripherals::take() {
        let clocks = Clocks::new(periph.CLOCK);
        let clocks = clocks.enable_ext_hfosc();
        let clocks = clocks.set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass);
        let clocks = clocks.start_lfclk();
        let _clocks = clocks.enable_ext_hfosc();
        //let board = hal::init().unwrap();

        let clocks = unsafe { CLOCKS.get_or_insert(_clocks) };

        let mut radio = {
            let mut radio = hal::ieee802154::Radio::init(periph.RADIO, clocks);

            // set TX power to its maximum value
            radio.set_txpower(ieee802154::TxPower::Pos8dBm);
            log::debug!("Radio initialized and configured with TX power set to the maximum value");
            radio
        };

        // these are the default settings of the DK's radio
        // NOTE if you ran `change-channel` then you may need to update the channel here
        radio.set_channel(Channel::_20); // <- must match the Dongle's listening channel
        radio.set_txpower(TxPower::Pos8dBm);

        let mut packet = Packet::new();

        // these three are equivalent
        // let msg: &[u8; 5] = &[72, 101, 108, 108, 111];
        // let msg: &[u8; 5] = &[b'H', b'e', b'l', b'l', b'o'];
        let msg: &[u8; 5] = b"Hello";

        log::info!(
            "sending: {}",
            str::from_utf8(msg).expect("msg is not valid UTF-8 data")
        );

        packet.copy_from_slice(msg);

        radio.send(&mut packet);
    }

    log::info!("`dk::exit() called; exiting ...`");
    
    // force any pending memory operation to complete before the BKPT instruction that follows
    atomic::compiler_fence(Ordering::SeqCst);
    loop {
        asm::bkpt()
    }
}
