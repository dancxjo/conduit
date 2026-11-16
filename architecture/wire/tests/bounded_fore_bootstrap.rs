//! Finite protocol scheduling proof, not physical UART timing or Start evidence.
//! MCU actions model the existing actor AFTER its separate genuine Started gate.
//! Every Hello/Ready is admitted by the real session machines and actual codec.
//! A send atomically queues its complete42-byte frame; a receive removes one.
//! This overapproximates unread UART bytes (partial framing can only drain more).
//! Scheduling is arbitrary between actions; the finite action deadline models
//! cancellation fencing, not elapsed wall time. It proves no physical timing.
//! Sequential Root admission may retain the other route's Hello, but emits that
//! route's Hello/Ready only once the previous session is active. At that point
//! at most its previous42-byte Ready remains unread, giving42+84=126bytes.
use conduit_wire::{decode_external_fore_session_frame,encode_external_fore_session_frame_into,
    ExternalForeSessionBinding as Binding,
    ExternalForeSessionMachine as Machine,ExternalForeSessionMessage as Message,
    SessionLimits,SessionMessage,SessionRole};
use std::collections::{HashSet,VecDeque};
const CAPACITY:usize=128;
const DEADLINE_STEPS:usize=64;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Strategy {Eager,ListenAll,Sequential}
#[derive(Clone,Debug)]
struct Packet {route:usize,hello:bool,bytes:Vec<u8>}
#[derive(Clone)]
struct Endpoint {machines:[Machine;2],hello:[bool;2],peer_hello:[bool;2],ready:[bool;2],peer_ready:[bool;2]}
impl Endpoint {
    fn new(root:bool)->Self {
        let machines=core::array::from_fn(|route|Machine::new(Binding {
            scope_commitment:[route as u8+1;32],
            limits:SessionLimits {maximum_in_flight_items:1,maximum_payload_bytes:837,maximum_buffered_bytes:1024},
            abnormal_kind:None,maximum_frame_bytes:1024,
        },if (route==0)==root {SessionRole::Source}else{SessionRole::Sink}).unwrap());
        Self {machines,hello:[false;2],peer_hello:[false;2],ready:[false;2],peer_ready:[false;2]}
    }
    fn send(&mut self,route:usize,hello:bool)->Packet {
        let message=if hello {Message::Hello}else{Message::Message(SessionMessage::Ready)};
        let frame=self.machines[route].binding().frame(message);
        self.machines[route].admit_outbound(frame).unwrap();
        let mut bytes=[0;1024];let n=encode_external_fore_session_frame_into(frame,&mut bytes,837,1024).unwrap();
        let mut framed=(n as u16).to_be_bytes().to_vec();framed.extend_from_slice(&bytes[..n]);
        if hello {self.hello[route]=true;}else{self.ready[route]=true;}
        Packet {route,hello,bytes:framed}
    }
    fn receive(&mut self,packet:Packet){
        let length=u16::from_be_bytes(packet.bytes[..2].try_into().unwrap()) as usize;
        assert_eq!(length,packet.bytes.len()-2);
        let frame=decode_external_fore_session_frame(&packet.bytes[2..],837,1024).unwrap();
        self.machines[packet.route].admit_inbound(frame).unwrap();
        if packet.hello {self.peer_hello[packet.route]=true;}else{self.peer_ready[packet.route]=true;}
    }
    fn active(&self)->bool {self.machines.iter().all(Machine::is_active)}
}
#[derive(Clone)]
struct Model {root:Endpoint,mcu:Endpoint,to_root:VecDeque<Packet>,to_mcu:VecDeque<Packet>,
    strategy:Strategy,stage:usize,cancelled:bool,trace:Vec<String>,peak:usize}
impl Model {
    fn new(strategy:Strategy)->Self {Self {root:Endpoint::new(true),mcu:Endpoint::new(false),
        to_root:VecDeque::new(),to_mcu:VecDeque::new(),strategy,stage:0,cancelled:false,trace:Vec::new(),peak:0}}
    fn pending(&self)->usize {self.to_mcu.iter().map(|p|p.bytes.len()).sum()}
    fn root_send(&mut self,route:usize,hello:bool)->Result<(),usize>{
        let packet=self.root.send(route,hello);assert_eq!(packet.bytes.len(),42);
        self.to_mcu.push_back(packet);let n=self.pending();self.peak=self.peak.max(n);
        self.trace.push(format!("Root {} {route}; MCU pending={n}",if hello {"Hello"}else{"Ready"}));
        if n>CAPACITY {Err(n)}else{Ok(())}
    }
    fn root_step(&mut self)->Result<bool,usize>{
        if self.cancelled {return Ok(false);}
        if self.strategy==Strategy::Eager {
            if let Some(route)=(0..2).find(|r|!self.root.hello[*r]) {self.root_send(route,true)?;return Ok(true);}
        }
        if self.strategy==Strategy::Sequential {
            while self.stage<2 && self.root.machines[self.stage].is_active(){self.stage+=1;}
            if self.stage<2 && self.root.peer_hello[self.stage] && !self.root.hello[self.stage] {
                self.root_send(self.stage,true)?;self.root_send(self.stage,false)?;return Ok(true);
            }
        }
        let Some(packet)=self.to_root.pop_front() else{return Ok(false)};
        let route=packet.route;let hello=packet.hello;self.root.receive(packet);
        self.trace.push(format!("Root receives {} {route}",if hello {"Hello"}else{"Ready"}));
        if hello && self.strategy!=Strategy::Sequential {
            if !self.root.hello[route] {self.root_send(route,true)?;}
            self.root_send(route,false)?;
        }
        Ok(true)
    }
    fn mcu_step(&mut self)->bool {
        if self.cancelled {return false;}
        // Existing send_pending checks each route's Hello/Ready before reading.
        for route in 0..2 {
            let hello=!self.mcu.hello[route];
            if hello || (self.mcu.peer_hello[route]&&!self.mcu.ready[route]) {
                let p=self.mcu.send(route,hello);self.to_root.push_back(p);assert!(self.to_root.iter().map(|p|p.bytes.len()).sum::<usize>()<=1024);
                self.trace.push(format!("MCU {} {route}",if hello {"Hello"}else{"Ready"}));return true;
            }
        }
        let Some(p)=self.to_mcu.pop_front() else{return false};
        self.trace.push(format!("MCU receives {} {}",if p.hello {"Hello"}else{"Ready"},p.route));
        self.mcu.receive(p);true
    }
    fn complete(&self)->bool {self.root.active()&&self.mcu.active()&&self.to_root.is_empty()&&self.to_mcu.is_empty()}
    fn key(&self)->String {format!("{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{}{:?}{:?}",self.root.hello,self.root.peer_hello,self.root.ready,self.root.peer_ready,
        self.mcu.hello,self.mcu.peer_hello,self.mcu.ready,self.mcu.peer_ready,self.stage,
        self.to_root.iter().map(|p|(p.route,p.hello)).collect::<Vec<_>>(),self.to_mcu.iter().map(|p|(p.route,p.hello)).collect::<Vec<_>>())}
}
#[derive(Default)]
struct ResultSet {states:usize,completed:usize,deadlocked:usize,peak:usize,overflow:Option<Vec<String>>}
fn explore(strategy:Strategy)->ResultSet {
    let mut result=ResultSet::default();let mut stack=vec![Model::new(strategy)];let mut seen=HashSet::new();
    while let Some(model)=stack.pop(){
        result.peak=result.peak.max(model.peak);
        if !seen.insert(model.key()){continue;}
        result.states+=1;
        assert!(model.trace.len()<DEADLINE_STEPS,"finite bootstrap deadline exceeded");
        if model.complete(){result.completed+=1;continue;}
        let mut root=model.clone();let r=root.root_step();
        match r {Err(_)=>{result.peak=result.peak.max(root.peak);result.overflow.get_or_insert(root.trace);},Ok(true)=>stack.push(root),Ok(false)=>{}}
        let mut mcu=model.clone();let m=mcu.mcu_step();if m {stack.push(mcu);}
        if matches!(r,Ok(false))&&!m {result.deadlocked+=1;}
    }
    result
}
#[test]
fn eager_and_unsequenced_listen_first_have_legal_168_byte_counterexamples(){
    for strategy in [Strategy::Eager,Strategy::ListenAll] {
        let result=explore(strategy);let trace=result.overflow.as_ref().expect("legal overflow schedule");
        assert_eq!(result.peak,168);assert_eq!(result.deadlocked,0);
        println!("{strategy:?}: states={} peak={} counterexample={trace:?}",result.states,result.peak);
    }
}
#[test]
fn sequential_listen_first_stays_at_126_bytes_and_terminates(){
    let result=explore(Strategy::Sequential);
    assert!(result.overflow.is_none());assert_eq!(result.deadlocked,0);assert!(result.completed>0);assert_eq!(result.peak,126);
    println!("Sequential: states={} completed={} peak={}",result.states,result.completed,result.peak);
}
#[test]
fn cancellation_or_deadline_never_manufactures_active_sessions(){
    let mut m=Model::new(Strategy::Sequential);assert!(m.mcu_step());assert!(m.root_step().unwrap());
    assert!(!m.root.active()&&!m.mcu.active());m.cancelled=true;
    let before=m.key();for _ in 0..DEADLINE_STEPS {assert!(!m.root_step().unwrap());assert!(!m.mcu_step());}
    assert_eq!(m.key(),before);assert!(!m.complete());
}

#[test]
fn scope_and_ready_order_are_not_relaxed_by_the_model(){
    let mut receiver=Endpoint::new(false);
    let mut frame=receiver.machines[0].binding().frame(Message::Hello);
    frame.scope_commitment=[99;32];
    assert!(receiver.machines[0].admit_inbound(frame).is_err());
    let ready=receiver.machines[0].binding().frame(Message::Message(SessionMessage::Ready));
    assert!(receiver.machines[0].admit_outbound(ready).is_err());
    assert!(!receiver.active());
}
