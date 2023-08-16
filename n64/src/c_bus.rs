use actor_framework::{OutboxSend, Time, Handler, Channel, Outbox};

use crate::{N64Actors, actors::{cpu_actor::{CpuOutbox, CpuActor}, rsp_actor::RspActor, rdp_actor::RdpActor, vi_actor::ViActor, ai_actor::AiActor, pi_actor::{PiActor, PiRead, PiWrite}, si_actor::SiActor, ri_actor::RiActor}};

/// CBus covers all devices that the CPU can access, other than RDRAM
/// This includes all MMIO mapped registers and mapped memory (RSP DMEM/IMEM, Cartridge ROM, Pif RAM)
///
/// These requests/responses go over the RCP's C-BUS, which is 32bits wide so they can complete even
/// if the D-BUS is busy DMAing data to/from RDRAM. But the C-BUS is also used by all devices to
/// start a DMA transfer to/from RDRAM.
///
/// C-BUS only supports aligned 32bit requests.
/// Other types of requests trigger various undefined behavior
///
/// As an optimization, CBus borrows resources from other actors, so it can handle requests instantly
/// without having to send messages.
///
pub struct CBus {
    dmem_imem: Option<Box<[u32; 0x800]>>,
    outstanding_request: Option<Outstanding>,
}

pub enum RegBusResult {
    /// The read completed instantly, no further action is needed
    ReadCompleted(u32),
    /// The write completed instantly, no further action is needed
    WriteCompleted,
    /// The request required synchronization with another actor and will automatically complete later
    Dispatched,
    /// The request hit unmapped memory, which is undefined behavior
    Unmapped,
}

enum Outstanding {
    Read(ReadFn, u32),
    Write(WriteFn, u32, u32),
}

enum HandlerResult {
    ReadCompleted(u32),
    WriteCompleted,
    Dispatched,
    Unmapped,
    Incomplete(Outstanding),
}

pub enum Resource {
    RspMem(Box<[u32; 2048]>),
}

pub enum ResourceRequest {
    RspMem,
}

pub struct ResourceReturnRequest {
    request: ResourceRequest,
    channel: Channel<N64Actors, CpuActor, Resource>,
}

impl ResourceReturnRequest {
    pub fn request<Out>(outbox: &mut Out, request: ResourceRequest, time: Time)
    where
        Out: Outbox<N64Actors> + OutboxSend<N64Actors, ResourceReturnRequest>,
        <Out as Outbox<N64Actors>>::Sender: Handler<N64Actors, Resource>,
    {
        outbox.send::<CpuActor>(Self {
            request,
            channel: Channel::<N64Actors, CpuActor, Resource>::new::<Out::Sender>(),
        }, time);
    }
}

pub struct CBusRead {
    pub address: u32
}

pub struct CBusWrite {
    pub address: u32,
    pub data: u32
}

impl CBus {
    pub fn new() -> Self {
        Self {
            dmem_imem: None,
            outstanding_request: None,
        }
    }

    /// Takes the HandlerResult from a handler and converts it into a RegBusResult
    ///
    /// Record the outstanding request
    fn handle(&mut self, result: HandlerResult) -> RegBusResult {
        match result {
            HandlerResult::ReadCompleted(data) => RegBusResult::ReadCompleted(data),
            HandlerResult::WriteCompleted => RegBusResult::WriteCompleted,
            HandlerResult::Dispatched => RegBusResult::Dispatched,
            HandlerResult::Unmapped => RegBusResult::Unmapped,
            HandlerResult::Incomplete(request) => {
                self.outstanding_request = Some(request);
                    RegBusResult::Dispatched
            }
        }
    }

    pub fn cpu_read(&mut self, outbox: &mut CpuOutbox, address: u32, time: Time) -> RegBusResult {
        let address_decode = (address >> 20) as usize;
        debug_assert!(address_decode >= 0x40, "Regbus read to RDRAM");

        if address_decode < 0x50 {
            let (read_fn, _) = HANDLERS[address_decode & 0xf];
            let result = (read_fn)(self, outbox, address, time);

            self.handle(result)
        } else if address_decode == 0x1fc {
            // SI External Bus
            // TODO: Don't send these as CpuRegRegs
            outbox.send::<SiActor>(CBusRead { address: address }, time);
            RegBusResult::Dispatched
        } else if address_decode < 0x800 {
            // PI External Bus
            outbox.send::<PiActor>(PiRead::new(address), time);
            RegBusResult::Dispatched
        } else {
            RegBusResult::Unmapped
        }
    }

    pub fn cpu_write(&mut self, outbox: &mut CpuOutbox, address: u32, data: u32, time: Time) -> RegBusResult {
        let address_decode = (address >> 20) as usize;
        debug_assert!(address_decode >= 0x40, "Regbus write to RDRAM");

        if address_decode < 0x50 {
            let (_, write_fn) = HANDLERS[address_decode & 0xf];
            let result = (write_fn)(self, outbox, address, data, time);
            self.handle(result)
        } else if address_decode == 0x1fc {
            // SI External Bus
            // TODO: Don't send these as CBusWrites
            outbox.send::<SiActor>(CBusWrite { address: address, data: data }, time);
            RegBusResult::Dispatched
        } else if address_decode < 0x800 {
            // PI External Bus
            outbox.send::<PiActor>(PiWrite::new(address, data), time);
            RegBusResult::Dispatched
        } else {
            RegBusResult::Unmapped
        }
    }

    pub fn receive_resource(&mut self, outbox: &mut CpuOutbox, resource: Resource, time: Time) -> RegBusResult {
        match resource {
            Resource::RspMem(mem) => {
                self.dmem_imem = Some(mem);
            }
        }
        match self.outstanding_request.take().unwrap() {
            Outstanding::Read(read_fn, address) => {
                let result = (read_fn)(self, outbox, address, time);
                self.handle(result)
            }
            Outstanding::Write(write_fn, address, data) => {
                let result = (write_fn)(self, outbox, address, data, time);
                self.handle(result)
            }
        }
    }

    pub fn return_resource(&mut self, outbox: &mut CpuOutbox, request: ResourceReturnRequest, time: Time) {
        match request.request {
            ResourceRequest::RspMem => {
                outbox.send::<RspActor>(Resource::RspMem(self.dmem_imem.take().unwrap()), time);
            }
        }
    }
}

type ReadFn = fn(resources: &mut CBus, outbox: &mut CpuOutbox, address: u32, time: Time) -> HandlerResult;
type WriteFn = fn(resources: &mut CBus, outbox: &mut CpuOutbox, address: u32, data: u32, time: Time) -> HandlerResult;

/// Read/Write Handlers for 0x04000000 - 0x05000000 range
static HANDLERS : [(ReadFn, WriteFn); 16] = [
    (read_rsp, write_rsp),
    (read_direct::<RdpActor>, write_direct::<RdpActor>),
    (read_unimplemented, write_unimplemented),
    (read_unimplemented, write_unimplemented),
    (read_direct::<ViActor>, write_direct::<ViActor>),
    (read_direct::<AiActor>, write_direct::<AiActor>),
    (read_direct::<PiActor>, write_direct::<PiActor>),
    (read_direct::<RiActor>, write_direct::<RiActor>),
    (read_direct::<SiActor>, write_direct::<SiActor>),
    (read_unmapped, write_unmapped),
    (read_unmapped, write_unmapped),
    (read_unmapped, write_unmapped),
    (read_unmapped, write_unmapped),
    (read_unmapped, write_unmapped),
    (read_unmapped, write_unmapped),
    (read_unmapped, write_unmapped),
];

fn read_rsp(resources: &mut CBus, outbox: &mut CpuOutbox, address: u32, time: Time) -> HandlerResult
{
    match address >> 18 & 0x3 {
        0 => {
            if let Some(mem) = resources.dmem_imem.as_mut() {
                let offset = ((address & 0x1ffc) >> 2) as usize;
                let data = mem[offset];
                HandlerResult::ReadCompleted(data)
            } else {
                // We don't currently have ownership of imem/dmem, need to request it from RspActor
                outbox.send::<RspActor>(ResourceRequest::RspMem, time);
                HandlerResult::Incomplete(Outstanding::Read(read_rsp, address))
            }
        }
        1 | 2 => { // 0x0404_0000 - 0x040b_ffff : RSP Registers
            outbox.send::<RspActor>(CBusRead { address: address }, time);
            HandlerResult::Dispatched
        }
        _ => {
            HandlerResult::Unmapped
        }
    }
}

fn write_rsp(resources: &mut CBus, outbox: &mut CpuOutbox, address: u32, data: u32, time: Time) -> HandlerResult
{
    match address >> 18 & 0x3 {
        0 => {
            if let Some(mem) = resources.dmem_imem.as_mut() {
                let offset = ((address & 0x1ffc) >> 2) as usize;
                mem[offset] = data;
                HandlerResult::WriteCompleted
            } else {
                // We don't currently have ownership of imem/dmem, need to request it from RspActor
                outbox.send::<RspActor>(ResourceRequest::RspMem, time);
                HandlerResult::Incomplete(Outstanding::Write(write_rsp, address, data))
            }
        }
        1 | 2 => { // 0x0404_0000 - 0x040b_ffff : RSP Registers
            outbox.send::<RspActor>(CBusWrite { address, data }, time);
            HandlerResult::Dispatched
        }
        _ => { HandlerResult::Unmapped }
    }
}

fn read_direct<Actor>(_: &mut CBus, outbox: &mut CpuOutbox, address: u32, time: Time) -> HandlerResult
where
    Actor: Handler<N64Actors, CBusRead>
{
    outbox.send::<Actor>(CBusRead { address: address }, time);
    HandlerResult::Dispatched
}

fn write_direct<Actor>(_: &mut CBus, outbox: &mut CpuOutbox, address: u32, data: u32, time: Time) -> HandlerResult
where
    Actor: Handler<N64Actors, CBusWrite>
{
    outbox.send::<Actor>(CBusWrite { address: address, data: data }, time);
    HandlerResult::Dispatched
}

fn read_unmapped(_: &mut CBus, _: &mut CpuOutbox, _: u32, _: Time) -> HandlerResult {
    HandlerResult::Unmapped
}

fn write_unmapped(_: &mut CBus, _: &mut CpuOutbox, _: u32, _: u32, _: Time) -> HandlerResult {
    HandlerResult::Unmapped
}

fn read_unimplemented(_: &mut CBus, _: &mut CpuOutbox, address: u32, _: Time) -> HandlerResult {
    todo!("Unimplemented read: {:08x}", address);
}

fn write_unimplemented(_: &mut CBus, _: &mut CpuOutbox, address: u32, data: u32, _: Time) -> HandlerResult {
    todo!("Unimplemented write: {:08x} = {:08x}", address, data);
}
