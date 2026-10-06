//! Divides remembered experience among partition engines.
//!
//! The engine a program talks to coordinates. It keeps current values,
//! answers, and outputs, along with a disposable copy of each memory's counts
//! and value for reading. Each experience lives in the partition chosen by a
//! hash of its canonical spelling. A question over a memory goes to every
//! partition, and the coordinator reduces their partial answers to the answer
//! one engine gives. In native builds each partition runs on its own thread.

use super::concept_answer::PartialAnswer;
use super::transport::ConceptGraph;
use super::{CompletionResult, ConceptId, ConceptKind, ConceptMap, Pangine};
use crate::Relevance;
use std::collections::BTreeSet;
use std::sync::Arc;

/// The partition engines behind a coordinator and the memories they hold.
pub(super) struct Partitions {
    hosts: Vec<Host>,
    threads: bool,
    // Percepts whose remembered experience lives in the partitions.
    memories: BTreeSet<ConceptId>,
}

enum Host {
    InThread(Box<Pangine>),
    #[cfg(not(target_arch = "wasm32"))]
    Worker(worker::Worker),
}

/// One unit of work for a partition engine. It holds only plain data, so it
/// can cross to a worker thread.
enum Request {
    /// Adds counts of experiences to a memory. It has no reply.
    Remember { memory: String, experiences: ConceptGraph, counts: Vec<i64> },
    /// Empties a memory. It has no reply.
    Forget { memory: String },
    /// Answers a question over memories as a partial answer.
    Answer { memories: Arc<Vec<String>>, question: Arc<ConceptGraph>, graded: bool },
    /// Returns a memory's experiences and their counts.
    Members { memory: String },
    /// Counts the distinct experiences in each memory.
    Sizes,
}

enum Reply {
    Part(Option<ConceptGraph>),
    Members(Option<(ConceptGraph, Vec<i64>)>),
    Sizes(Vec<(String, usize)>),
}

impl Partitions {
    fn new(count: usize, threads: bool) -> Self {
        Self { hosts: (0..count).map(|_| Host::new(threads)).collect(), threads, memories: BTreeSet::new() }
    }

    fn len(&self) -> usize {
        self.hosts.len()
    }

    /// Sends a request that replies to every partition and returns the
    /// replies in partition order. Every worker thread gets its request
    /// before any reply is read, so the workers answer at the same time.
    fn broadcast(&mut self, request: impl Fn() -> Request) -> Vec<Reply> {
        #[cfg(not(target_arch = "wasm32"))]
        for host in &self.hosts {
            if let Host::Worker(worker) = host {
                worker.send(request());
            }
        }
        self.hosts
            .iter_mut()
            .filter_map(|host| match host {
                Host::InThread(engine) => serve(engine, request()),
                #[cfg(not(target_arch = "wasm32"))]
                Host::Worker(worker) => Some(worker.receive()),
            })
            .collect()
    }

    /// Sends a request without a reply to one partition. A worker thread
    /// works through its requests in order, so a later question sees it.
    fn tell(&mut self, index: usize, request: Request) {
        match &mut self.hosts[index] {
            Host::InThread(engine) => {
                serve(engine, request);
            }
            #[cfg(not(target_arch = "wasm32"))]
            Host::Worker(worker) => worker.send(request),
        }
    }
}

impl Host {
    fn new(threads: bool) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        if threads {
            return Self::Worker(worker::Worker::spawn());
        }
        #[cfg(target_arch = "wasm32")]
        let _ = threads;
        Self::InThread(Box::new(Pangine::new()))
    }
}

// Runs one request in a partition engine and returns its reply, if it has one.
fn serve(engine: &mut Pangine, request: Request) -> Option<Reply> {
    match request {
        Request::Remember { memory, experiences, counts } => {
            let memory = engine.reference_percept(&memory);
            for (experience, count) in engine.import_graphs(&experiences).unwrap_or_default().iter().zip(counts) {
                engine.add_experience(&memory, experience, Relevance::new(count));
            }
            None
        }
        Request::Forget { memory } => {
            let memory = engine.reference_percept(&memory);
            engine.set_percept_subconcepts(&memory, ConceptMap::new());
            None
        }
        Request::Answer { memories, question, graded } => {
            let sources = memories.iter().map(|memory| engine.reference_percept(memory)).collect::<Vec<_>>();
            let part = engine.import_graph(&question).and_then(|question| engine.partial_answer(&sources, &question, graded)).and_then(|part| {
                let encoded = part.encode(engine);
                engine.export_graph(&encoded)
            });
            Some(Reply::Part(part))
        }
        Request::Members { memory } => {
            let memory = engine.reference_percept(&memory);
            let (counts, experiences): (Vec<_>, Vec<_>) =
                engine.get_relevance_map(&memory).into_iter().map(|(count, experience)| (count.count(), experience)).unzip();
            Some(Reply::Members(engine.export_graphs(&experiences).map(|graph| (graph, counts))))
        }
        Request::Sizes => Some(Reply::Sizes(engine.memory_sizes())),
    }
}

/// Routes an experience by a 64-bit FNV-1a hash of its canonical spelling,
/// which native and browser builds compute alike. The lowest bit of FNV-1a
/// depends only on how many of the spelling's bytes are odd, so with an even
/// partition count, spellings that differ only in names they each repeat
/// twice, as families do, would fill only half the partitions. MurmurHash3's
/// 64-bit finalizer mixes every bit before the partition is chosen.
pub(super) fn route(spelling: &str, partitions: usize) -> usize {
    let mut hash = spelling.bytes().fold(0xCBF2_9CE4_8422_2325_u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3));
    hash = (hash ^ (hash >> 33)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    hash = (hash ^ (hash >> 33)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    hash ^= hash >> 33;
    (hash % partitions as u64) as usize
}

fn percept_name(percept: &ConceptId) -> Option<&str> {
    match &percept.0.kind {
        ConceptKind::Percept { name } => Some(name),
        _ => None,
    }
}

// Partitioned memory.
impl Pangine {
    /// Divides remembered experience among `count` partition engines.
    ///
    /// Each experience then lives in the partition chosen by a hash of its
    /// canonical spelling, so its whole count stays together. A question over
    /// a memory goes to every partition, and this engine reduces their
    /// partial answers to the answer it would give alone. It keeps current
    /// values, answers, and a disposable copy of each memory's counts and
    /// value for reading. In native builds each partition runs on its own
    /// thread; in the browser, partitions take turns.
    ///
    /// Calling it again divides the experience anew, and one partition brings
    /// everything back into this engine. Returns false for zero.
    pub fn set_partitions(&mut self, count: usize) -> bool {
        self.divide_memory(count, cfg!(not(target_arch = "wasm32")))
    }

    /// Returns how many partitions hold this engine's remembered experience.
    pub fn partition_count(&self) -> usize {
        self.partitions.as_ref().map_or(1, |partitions| partitions.len())
    }

    /// Loses one partition and the experience it holds, as if its machine
    /// were replaced by an empty one.
    ///
    /// This engine rebuilds its copy of every memory from the partitions that
    /// remain. Answers already given keep their values, later questions
    /// answer from the remaining experience, and new experience routes as
    /// before. Returns false when there is no partition engine at `index`.
    pub fn drop_partition(&mut self, index: usize) -> bool {
        let Some(mut partitions) = self.partitions.take() else {
            return false;
        };
        if index >= partitions.len() {
            self.partitions = Some(partitions);
            return false;
        }

        partitions.hosts[index] = Host::new(partitions.threads);
        for memory in partitions.memories.clone() {
            let Some(name) = percept_name(&memory).map(str::to_owned) else {
                continue;
            };
            let mut members = ConceptMap::new();
            for reply in partitions.broadcast(|| Request::Members { memory: name.clone() }) {
                let Reply::Members(Some((graph, counts))) = reply else {
                    continue;
                };
                for (experience, count) in self.import_graphs(&graph).unwrap_or_default().into_iter().zip(counts) {
                    members.insert(experience, Relevance::new(count));
                }
            }
            self.write_memory_copy(&memory, members);
        }
        self.partitions = Some(partitions);
        true
    }

    /// Returns the partition an owned Concept routes to as an experience.
    pub fn partition_of(&self, experience: &ConceptId) -> Option<usize> {
        self.owns(experience).then(|| route(&self.format_concept(experience, false), self.partition_count()))
    }

    fn divide_memory(&mut self, count: usize, threads: bool) -> bool {
        if count == 0 {
            return false;
        }
        // This engine's copy of every memory is complete, so the old
        // partitions can go before the experience is divided anew.
        self.partitions = None;
        if count == 1 {
            return true;
        }

        let mut partitions = Partitions::new(count, threads);
        for memory in self.remembered_memories() {
            let members = self.percept_subconcepts.get(&memory.index()).cloned().unwrap_or_default();
            self.route_experiences(&mut partitions, &memory, members.iter().map(|(experience, count)| (experience.clone(), *count)));
            partitions.memories.insert(memory);
        }
        self.partitions = Some(Box::new(partitions));
        true
    }

    /// Sends an experience this engine just recorded to its partition. A
    /// Percept that becomes a memory brings everything it already holds, as
    /// one engine turns a value into remembered experience.
    pub(super) fn remember_in_partitions(&mut self, memory: &ConceptId, experience: &ConceptId) {
        let Some(mut partitions) = self.partitions.take() else {
            return;
        };
        if partitions.memories.insert(memory.clone()) {
            let members = self.percept_subconcepts.get(&memory.index()).cloned().unwrap_or_default();
            self.route_experiences(&mut partitions, memory, members.iter().map(|(experience, count)| (experience.clone(), *count)));
        } else {
            self.route_experiences(&mut partitions, memory, [(experience.clone(), Relevance::DEFAULT)]);
        }
        self.partitions = Some(partitions);
    }

    /// Takes a replaced memory out of the partitions.
    pub(super) fn forget_partitioned_memory(&mut self, memory: &ConceptId) {
        let Some(partitions) = self.partitions.as_mut() else {
            return;
        };
        if !partitions.memories.remove(memory) {
            return;
        }
        if let Some(name) = percept_name(memory) {
            for index in 0..partitions.len() {
                partitions.tell(index, Request::Forget { memory: name.to_owned() });
            }
        }
    }

    pub(super) fn selects_partitioned_memory(&self, sources: &[ConceptId]) -> bool {
        self.partitions.as_ref().is_some_and(|partitions| sources.iter().any(|source| partitions.memories.contains(source)))
    }

    /// Answers a question over sources that include partitioned memories.
    /// Every partition answers over its share of those memories, this engine
    /// answers over the other selected Percepts, and the partial answers
    /// reduce.
    pub(super) fn complete_partitioned(&mut self, sources: &[ConceptId], question: &ConceptId, graded: bool) -> Option<CompletionResult> {
        if !self.valid_question_sources(sources, question) {
            return None;
        }

        let mut partitions = self.partitions.take()?;
        let (memories, local): (Vec<_>, Vec<_>) = sources.iter().cloned().partition(|source| partitions.memories.contains(source));
        let names = Arc::new(memories.iter().filter_map(|memory| percept_name(memory).map(str::to_owned)).collect::<Vec<_>>());
        let replies = self
            .export_graph(question)
            .map(Arc::new)
            .map(|question| partitions.broadcast(|| Request::Answer { memories: Arc::clone(&names), question: Arc::clone(&question), graded }));
        self.partitions = Some(partitions);

        let mut parts = Vec::new();
        for reply in replies? {
            let Reply::Part(Some(graph)) = reply else {
                return None;
            };
            let encoded = self.import_graph(&graph)?;
            parts.push(PartialAnswer::decode(self, &encoded)?);
        }
        if !local.is_empty() {
            parts.push(self.partial_answer(&local, question, graded)?);
        }
        self.reduce_partial_answers(question, parts, graded)
    }

    /// Lists, for each partition, the distinct experiences each memory holds
    /// there.
    pub(super) fn partition_contents(&mut self) -> Vec<Vec<(String, usize)>> {
        let Some(partitions) = self.partitions.as_mut() else {
            return vec![self.memory_sizes()];
        };
        partitions.broadcast(|| Request::Sizes).into_iter().map(|reply| if let Reply::Sizes(sizes) = reply { sizes } else { Vec::new() }).collect()
    }

    // Sends experiences, with their counts, to the partitions their spellings
    // route to.
    fn route_experiences(&self, partitions: &mut Partitions, memory: &ConceptId, experiences: impl IntoIterator<Item = (ConceptId, Relevance)>) {
        let Some(name) = percept_name(memory) else {
            return;
        };
        let mut batches = (0..partitions.len()).map(|_| (Vec::new(), Vec::new())).collect::<Vec<_>>();
        for (experience, count) in experiences {
            let (experiences, counts) = &mut batches[route(&self.format_concept(&experience, false), partitions.len())];
            experiences.push(experience);
            counts.push(count.count());
        }
        for (index, (experiences, counts)) in batches.into_iter().enumerate() {
            if experiences.is_empty() {
                continue;
            }
            if let Some(graph) = self.export_graphs(&experiences) {
                partitions.tell(index, Request::Remember { memory: name.to_owned(), experiences: graph, counts });
            }
        }
    }

    // Percepts whose value is remembered experience rather than a current
    // value or an answer.
    fn remembered_memories(&self) -> Vec<ConceptId> {
        self.percepts
            .values()
            .filter(|percept| {
                self.is_mutable_percept(percept)
                    && !self.current_value_percepts.contains(&percept.index())
                    && self.percept_subconcepts.get(&percept.index()).is_some_and(|members| !members.is_empty())
                    && self.live_answer_value(percept).is_none()
            })
            .cloned()
            .collect()
    }

    // Counts the distinct experiences in each remembered memory, by name.
    fn memory_sizes(&self) -> Vec<(String, usize)> {
        self.remembered_memories()
            .iter()
            .filter_map(|memory| Some((percept_name(memory)?.to_owned(), self.percept_subconcepts.get(&memory.index())?.len())))
            .collect()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod worker {
    use super::{serve, Pangine, Reply, Request};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::thread::JoinHandle;

    /// A partition engine on its own thread. The engine is created on that
    /// thread and never leaves it; requests and replies cross as plain data.
    pub(super) struct Worker {
        requests: Option<Sender<Request>>,
        replies: Receiver<Reply>,
        thread: Option<JoinHandle<()>>,
    }

    impl Worker {
        pub(super) fn spawn() -> Self {
            let (requests, incoming) = channel::<Request>();
            let (outgoing, replies) = channel::<Reply>();
            let thread = std::thread::Builder::new()
                .name("pangine-partition".to_owned())
                .spawn(move || {
                    let mut engine = Pangine::new();
                    for request in incoming {
                        if let Some(reply) = serve(&mut engine, request) {
                            if outgoing.send(reply).is_err() {
                                break;
                            }
                        }
                    }
                })
                .expect("a partition thread starts");
            Self { requests: Some(requests), replies, thread: Some(thread) }
        }

        pub(super) fn send(&self, request: Request) {
            self.requests.as_ref().expect("an open partition").send(request).expect("a running partition thread");
        }

        pub(super) fn receive(&self) -> Reply {
            self.replies.recv().expect("a running partition thread")
        }
    }

    impl Drop for Worker {
        fn drop(&mut self) {
            // Closing the request channel ends the thread's loop.
            self.requests.take();
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every way a question meets remembered experience: repeated and inverted
    // experience, a join through a shared blank, composed and generalized
    // rows, an exact row whose clause groups connect through one experience,
    // adjustments, choices and draws, a value that turns into memory, a
    // replaced memory, and a question over two memories at once.
    const SCRIPT: &str = "
{world} ~= [morning]->[birds]
{world} ~= [morning]->[birds]
{world} ~= [morning]->[traffic]
{world} @ [morning]->{answer}
${answer}
{knowledge} ~= [Socrates]->[is-a]->[human]
{knowledge} ~= [human]->[is-a]->[mortal]
{knowledge} @ ([Socrates]->[is-a]->{kind})({kind}->[is-a]->{conclusion})
{closet} ~= ([top]->[red])([bottom]->[jeans])
{closet} ~= ([top]->[blue])([bottom]->[skirt])
{closet} ~= [top]->[green]
{closet} @~ ([top]->{shirt})([bottom]->{pants})
$({shirt}->{pants})
{families} ~= ([Tom]->[parent-of]->[Bob])([Bob]->[parent-of]->[Ann])([Tom]->[grandparent-of]->[Ann])
{families} ~= ([Liz]->[parent-of]->[Max])([Max]->[parent-of]->[Ivy])([Liz]->[grandparent-of]->[Ivy])
{families} @~ ([Joe]->[parent-of]->[Sue])([Sue]->[parent-of]->[Kim])([Joe]->[grandparent-of]->{grandchild})
{pieces} ~= [a]->[r]->[b]
{pieces} ~= ([b]->[s]->[c])([d]->[t]->[q])
{pieces} ~= !([a]->[r]->[b])
{pieces} ~= [a]->[r]->[b]
{pieces} @ ({x}->[r]->{y})({y}->[s]->{z})({w}->[t]->[q])
{candidates} ~= [A]
{candidates} ~= [B]
{helpful} ~= [A]
{failed} ~= [A]
{failed} ~= [A]
{candidates} @ {choice}
{helpful} @ {helpful-choice}
{choice} @+= {helpful-choice}
{failed} @ {failed-choice}
{choice} @-= {failed-choice}
^{choice}
{drinks} ~= [tea]
{drinks} ~= [coffee]
{drinks} ~= [coffee]
{drinks} @ {drink}
^~{drink}
{mixed} = [current]
{mixed} ~= [remembered]
{mixed} @ {item}
{replaced} ~= [old]
{replaced} = [new]
{replaced} @ {kept}
{world}{knowledge} @ {anything}->{relation}->{something}
{world} ~= [evening]->[crickets]
{world} @ {time}->{sound}
";

    const INSPECTED: [&str; 9] =
        ["{answer}", "{conclusion}", "{shirt}->{pants}", "{grandchild}", "{x}->{y}->{z}->{w}", "{item}", "{kept}", "{something}", "{time}->{sound}"];

    #[test]
    fn divided_memory_answers_every_statement_as_one_engine_does() {
        let mut whole = Pangine::new();
        let expected = (transcript(&mut whole, SCRIPT), inspections(&mut whole));
        for threads in [false, true] {
            for count in [2, 3, 7, 16] {
                let mut divided = Pangine::new();
                assert!(divided.divide_memory(count, threads));
                let actual = (transcript(&mut divided, SCRIPT), inspections(&mut divided));
                assert!(actual == expected, "{count} partitions, threads {threads}:\n{actual:#?}");
            }
        }
    }

    #[test]
    fn dividing_anew_midway_changes_no_answer() {
        let (first, second) = SCRIPT.split_at(SCRIPT.find("{candidates}").expect("a second half"));
        let mut whole = Pangine::new();
        let expected = transcript(&mut whole, first) + &transcript(&mut whole, second);
        for [before, after] in [[3, 5], [4, 1], [1, 2]] {
            let mut divided = Pangine::new();
            divided.divide_memory(before, false);
            let mut actual = transcript(&mut divided, first);
            divided.divide_memory(after, true);
            actual += &transcript(&mut divided, second);
            assert_eq!(actual, expected, "{before} partitions, then {after}");
        }
    }

    #[test]
    fn losing_a_partition_leaves_the_remaining_experience() {
        let experiences = (0..12).map(|index| format!("[item-{index}]->[r]->[value-{}]", index % 3)).collect::<Vec<_>>();
        let mut divided = Pangine::new();
        assert!(divided.divide_memory(3, true));
        for experience in &experiences {
            run(&mut divided, &format!("{{memory}} ~= {experience}"));
        }
        let before = run(&mut divided, "{memory} @ {x}->[r]->{y}");
        let answered = run(&mut divided, "${y}");

        let lost = 1;
        let remaining = experiences
            .iter()
            .filter(|experience| {
                let concept = must_ref(&mut divided, experience);
                divided.partition_of(&concept) != Some(lost)
            })
            .collect::<Vec<_>>();
        assert!(!remaining.is_empty() && remaining.len() < experiences.len(), "the lost partition holds some of the experience");
        assert!(divided.drop_partition(lost));
        assert!(!divided.drop_partition(3));
        assert_eq!(run(&mut divided, "${y}"), answered, "an answer already given keeps its value");

        let mut survivor = Pangine::new();
        for experience in &remaining {
            run(&mut survivor, &format!("{{memory}} ~= {experience}"));
        }
        assert_eq!(run(&mut divided, "${memory}"), run(&mut survivor, "${memory}"));
        let after = run(&mut divided, "{memory} @ {x}->[r]->{y}");
        assert_eq!(after, run(&mut survivor, "{memory} @ {x}->[r]->{y}"));
        assert_ne!(after, before);
    }

    #[test]
    fn replacing_a_memory_takes_it_out_of_the_partitions() {
        let mut divided = Pangine::new();
        assert!(divided.divide_memory(3, false));
        for statement in ["{memory} ~= [a]", "{memory} ~= [b]", "{memory} ~= [c]"] {
            run(&mut divided, statement);
        }
        assert_eq!(held(&mut divided), 3);

        run(&mut divided, "{memory} = [d]");
        assert_eq!(held(&mut divided), 0, "the partitions no longer hold the memory");
        assert_eq!(run(&mut divided, "{memory} @ {x}"), "[d]");

        // Remembering again turns the current value into experience.
        run(&mut divided, "{memory} ~= [e]");
        assert_eq!(held(&mut divided), 2);
        assert_eq!(run(&mut divided, "{memory} @ {x}"), "[d][e]");
    }

    #[test]
    fn an_experience_routes_by_its_canonical_spelling() {
        let mut pangine = Pangine::new();
        let concept = must_ref(&mut pangine, "[b][a]");
        assert_eq!(pangine.partition_of(&concept), Some(0), "one partition holds everything");
        assert!(pangine.set_partitions(5));
        let same = must_ref(&mut pangine, "[a][b]");
        assert_eq!(pangine.partition_of(&concept), pangine.partition_of(&same));
        assert_eq!(pangine.partition_of(&concept), Some(route("[a][b]", 5)));
        assert_eq!(Pangine::new().partition_of(&concept), None, "a foreign Concept routes nowhere");
        assert!(!pangine.set_partitions(0));
        assert_eq!(pangine.partition_count(), 5);
    }

    fn transcript(pangine: &mut Pangine, script: &str) -> String {
        let mut details = Vec::new();
        pangine.parse_script_text_with_details(script, &mut details).expect("a valid script");
        String::from_utf8(details).expect("UTF-8 details")
    }

    fn inspections(pangine: &mut Pangine) -> Vec<Option<Result<Vec<String>, String>>> {
        INSPECTED.iter().map(|operand| pangine.debug_console_command(&format!("inspect {operand}"))).collect()
    }

    // The distinct experiences that the partitions hold, in all.
    fn held(pangine: &mut Pangine) -> usize {
        pangine.partition_contents().iter().flatten().map(|(_, experiences)| experiences).sum()
    }

    fn run(pangine: &mut Pangine, statement: &str) -> String {
        let value = pangine.reference_concept(statement).unwrap_or_else(|error| panic!("failed to run {statement:?}: {error}"));
        value.map_or_else(|| "[]".to_owned(), |value| pangine.format_concept(&value, false))
    }

    fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
        pangine
            .reference_concept(input)
            .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
            .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
    }
}
