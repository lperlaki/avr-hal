use core::marker::PhantomData;

use avr_device::interrupt::{self, CriticalSection, Mutex};

pub struct UsbBus {
    usb: Mutex<crate::pac::USB_DEVICE>,
    pll: Mutex<crate::pac::PLL>,
}

impl UsbBus {
    pub fn new(usb: crate::pac::USB_DEVICE, pll: crate::pac::PLL) -> Self {
        Self {
            usb: Mutex::new(usb),
            pll: Mutex::new(pll),
        }
    }

    // Needs bare-metal = 1.0
    // pub fn release(self) -> crate::atmega32u4::USB_DEVICE {
    //     self.usb.into_inner()
    // }
}
use usb_device::{
    bus::PollResult,
    endpoint::{EndpointAddress, EndpointType},
    Result, UsbDirection, UsbError,
};

struct Selected;
struct NotSelected;

struct Endpoint<'bus, S = Selected> {
    address: EndpointAddress,
    bus: &'bus UsbBus,
    _selected: PhantomData<S>,
}

impl<'bus, S> Endpoint<'bus, S> {
    fn new(bus: &'bus UsbBus, address: EndpointAddress) -> Result<Endpoint<'bus, NotSelected>> {
        let ep = Endpoint {
            address,
            bus,
            _selected: PhantomData,
        };
        if !matches!(ep.as_byte(), 0b000..=0b110) {
            return Err(UsbError::InvalidEndpoint);
        }
        Ok(ep)
    }

    fn select<'cs>(self, cs: &'cs CriticalSection) -> Endpoint<'bus, Selected> {
        let usb = self.bus.usb.borrow(cs);
        usb.uenum.write(|w| unsafe { w.bits(self.as_byte()) });
        Endpoint {
            bus: self.bus,
            address: self.address,
            _selected: PhantomData,
        }
    }
    fn as_byte(&self) -> u8 {
        u8::from(self.address) & !(UsbDirection::In as u8)
    }
}

impl<'bus> Endpoint<'bus> {
    fn enable<'cs>(&self, cs: &'cs CriticalSection) {
        let usb = self.bus.usb.borrow(cs);
        usb.ueconx.modify(|_, w| w.epen().set_bit())
    }

    fn disable<'cs>(&self, cs: &'cs CriticalSection) {
        let usb = self.bus.usb.borrow(cs);
        usb.ueconx.modify(|_, w| w.epen().clear_bit())
    }

    fn is_enabled<'cs>(&self, cs: &'cs CriticalSection) -> bool {
        let usb = self.bus.usb.borrow(cs);
        usb.ueconx.read().epen().bit_is_set()
    }

    fn alloc<'cs>(&self, cs: &'cs CriticalSection) {
        let usb = self.bus.usb.borrow(cs);
        usb.uecfg1x.modify(|_, w| w.alloc().set_bit())
    }

    fn dealloc<'cs>(&self, cs: &'cs CriticalSection) {
        let usb = self.bus.usb.borrow(cs);
        usb.uecfg1x.modify(|_, w| w.alloc().clear_bit())
    }

    fn is_alloced<'cs>(&self, cs: &'cs CriticalSection) -> bool {
        let usb = self.bus.usb.borrow(cs);
        usb.uecfg1x.read().alloc().bit_is_set()
    }

    fn set_direction<'cs>(&self, cs: &'cs CriticalSection, direction: UsbDirection) {
        let usb = self.bus.usb.borrow(cs);
        usb.uecfg0x.modify(|_, w| match direction {
            UsbDirection::Out => w.epdir().clear_bit(),
            UsbDirection::In => w.epdir().set_bit(),
        })
    }

    fn set_typ<'cs>(&self, cs: &'cs CriticalSection, typ: EndpointType) {
        let usb = self.bus.usb.borrow(cs);
        usb.uecfg0x.modify(|_, w| w.eptype().bits(typ as u8))
    }

    fn set_size<'cs>(&self, cs: &'cs CriticalSection, size: u16) -> Result<()> {
        let usb = self.bus.usb.borrow(cs);
        let size = match size {
            0..=8 => 0b000,
            9..=16 => 0b001,
            17..=32 => 0b010,
            33..=64 => 0b011,
            65..=128 => 0b100,
            129..=256 => 0b101,
            257..=512 => 0b110,
            _ => return Err(UsbError::EndpointMemoryOverflow),
        };
        usb.uecfg1x
            .write(|w| w.epsize().bits(size).alloc().set_bit());
        Ok(())
    }

    fn get_size<'cs>(&self, cs: &'cs CriticalSection) -> Result<u16> {
        let usb = self.bus.usb.borrow(cs);
        let size = match usb.uecfg1x.read().epsize().bits() {
            0b000 => 8,
            0b001 => 16,
            0b010 => 32,
            0b011 => 64,
            0b100 => 128,
            0b101 => 256,
            0b110 => 512,
            _ => return Err(UsbError::EndpointMemoryOverflow),
        };
        Ok(size)
    }

    fn is_cfg_ok<'cs>(&self, cs: &'cs CriticalSection) -> bool {
        let usb = self.bus.usb.borrow(cs);
        usb.uesta0x.read().cfgok().bit_is_set()
    }

    fn is_stalled<'cs>(&self, cs: &'cs CriticalSection) -> bool {
        let usb = self.bus.usb.borrow(cs);
        usb.ueconx.read().stallrq().bit_is_set()
    }

    fn set_stalled<'cs>(&self, cs: &'cs CriticalSection, stalled: bool) {
        let usb = self.bus.usb.borrow(cs);
        usb.ueconx.modify(|_, w| match stalled {
            true => w.stallrq().set_bit(),
            false => w.stallrqc().set_bit(),
        })
    }

    fn read<'cs>(&self, cs: &'cs CriticalSection, buf: &mut [u8]) -> Result<usize> {
        let usb = self.bus.usb.borrow(cs);
        usb.ueintx.modify(|_, w| w.rxouti().clear_bit());
        let mut i = 0;
        while usb.ueintx.read().rwal().bit_is_set() && i < buf.len() {
            buf[i] = usb.uedatx.read().dat().bits();
            i += 1;
        }
        usb.ueintx.modify(|_, w| w.fifocon().clear_bit());
        Ok(i)
    }

    fn can_read<'cs>(&self, cs: &'cs CriticalSection) -> bool {
        let usb = self.bus.usb.borrow(cs);
        usb.ueintx.read().rxouti().bit_is_set()
    }

    fn write<'cs>(&self, cs: &'cs CriticalSection, buf: &[u8]) -> Result<usize> {
        let usb = self.bus.usb.borrow(cs);
        usb.ueintx.modify(|_, w| w.txini().clear_bit());
        let mut i = 0;
        while usb.ueintx.read().rwal().bit_is_set() && i < buf.len() {
            usb.uedatx.write(|w| w.dat().bits(buf[i]));
            i += 1;
        }
        usb.ueintx.modify(|_, w| w.fifocon().clear_bit());
        Ok(i)
    }

    fn can_write<'cs>(&self, cs: &'cs CriticalSection) -> bool {
        let usb = self.bus.usb.borrow(cs);
        usb.ueintx.read().txini().bit_is_set()
    }
}

impl UsbBus {
    fn get_ep(&self, ep_addr: EndpointAddress) -> Result<Endpoint<'_, NotSelected>> {
        Endpoint::<NotSelected>::new(self, ep_addr)
    }

    fn iter_endpoints(&self) -> impl Iterator<Item = Endpoint<'_, NotSelected>> {
        (0b000..=0b110)
            .map(EndpointAddress::from)
            .filter_map(move |a| self.get_ep(a).ok())
    }
}

impl usb_device::bus::UsbBus for UsbBus {
    fn alloc_ep(
        &mut self,
        ep_dir: UsbDirection,
        ep_addr: Option<EndpointAddress>,
        ep_type: EndpointType,
        max_packet_size: u16,
        _interval: u8,
    ) -> Result<EndpointAddress> {
        let addr = ep_addr.unwrap_or(1.into());

        interrupt::free(|cs| {
            let ep = self.get_ep(addr)?.select(cs);
            ep.enable(cs);
            ep.set_direction(cs, ep_dir);
            ep.set_typ(cs, ep_type);
            ep.set_size(cs, max_packet_size)?;
            ep.alloc(cs);

            if ep.is_cfg_ok(cs) {
                Ok(addr)
            } else {
                Err(UsbError::InvalidEndpoint)
            }
        })
    }

    fn enable(&mut self) {
        interrupt::free(|cs| {
            let usb = self.usb.borrow(cs);
            let pll = self.pll.borrow(cs);

            // # Power On the USB interface
            //  Power-On USB pads regulator
            usb.uhwcon.modify(|_, w| w.uvrege().set_bit());
            //  Configure PLL interface
            // TODO
            //  Enable PLL
            pll.pllcsr.modify(|_, w| w.plle().set_bit());
            //  Check PLL lock
            while pll.pllcsr.read().plock().bit_is_clear() {}
            //  Enable USB interface
            usb.usbcon.modify(|_, w| w.usbe().set_bit());
            // Taken care of by usb-device impl
            //  Configure USB interface (USB speed, Endpoints configuration...)
            //  Wait for USB VBUS information connection
            //  Attach USB device
        })
    }

    fn reset(&self) {
        interrupt::free(|cs| {
            let usb = self.usb.borrow(cs);
            usb.usbcon.modify(|_, w| w.usbe().clear_bit());
        })
    }

    fn set_device_address(&self, addr: u8) {
        interrupt::free(|cs| {
            let usb = self.usb.borrow(cs);

            usb.udaddr.modify(|_, w| w.uadd().bits(addr));
            usb.udaddr.modify(|_, w| w.adden().set_bit());
        })
    }

    fn write(&self, ep_addr: EndpointAddress, buf: &[u8]) -> Result<usize> {
        interrupt::free(|cs| {
            let ep = self.get_ep(ep_addr)?.select(cs);
            if !ep.can_write(cs) {
                return Err(UsbError::WouldBlock);
            }
            if buf.len() as u16 > ep.get_size(cs)? {
                return Err(UsbError::BufferOverflow);
            }

            ep.write(cs, buf)
        })
    }

    fn read(&self, ep_addr: EndpointAddress, buf: &mut [u8]) -> Result<usize> {
        interrupt::free(|cs| {
            let ep = self.get_ep(ep_addr)?.select(cs);
            if !ep.can_read(cs) {
                return Err(UsbError::WouldBlock);
            }
            if buf.len() as u16 > ep.get_size(cs)? {
                return Err(UsbError::BufferOverflow);
            }
            ep.read(cs, buf)
        })
    }

    fn set_stalled(&self, ep_addr: EndpointAddress, stalled: bool) {
        interrupt::free(|cs| {
            self.get_ep(ep_addr)
                .unwrap()
                .select(cs)
                .set_stalled(cs, stalled)
        })
    }

    fn is_stalled(&self, ep_addr: EndpointAddress) -> bool {
        interrupt::free(|cs| self.get_ep(ep_addr).unwrap().select(cs).is_stalled(cs))
    }

    fn suspend(&self) {
        interrupt::free(|cs| {
            let usb = self.usb.borrow(cs);
            let pll = self.pll.borrow(cs);

            // # Suspending the USB interface
            //  Clear Suspend Bit
            usb.udint.modify(|_, w| w.suspi().clear_bit());
            //  Freeze USB clock
            usb.usbcon.modify(|_, w| w.frzclk().set_bit());
            //  Disable PLL
            pll.pllcsr.modify(|_, w| w.plle().clear_bit());
            //  Be sure to have interrupts enable to exit sleep mode
            usb.udien.modify(|_, w| w.wakeupe().set_bit());
            //  Make the MCU enter sleep mode
            //TODO
        })
    }

    fn resume(&self) {
        interrupt::free(|cs| {
            let usb = self.usb.borrow(cs);
            let pll = self.pll.borrow(cs);

            // # Resuming the USB interface
            //  Enable PLL
            pll.pllcsr.modify(|_, w| w.plle().set_bit());
            //  Wait PLL lock
            while pll.pllcsr.read().plock().bit_is_clear() {}
            //  Unfreeze USB clock
            usb.usbcon.modify(|_, w| w.frzclk().clear_bit());
            //  Clear Resume information
            // TODO
        })
    }

    fn poll(&self) -> PollResult {
        PollResult::None
    }
}
