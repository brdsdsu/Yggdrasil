# Distributed Database Reading List for Yggdrasil

A two-week reading plan aimed at Yggdrasil's problem: several agents updating a shared, layered
scene graph, often over unreliable links. For that, merge-based conflict resolution matters as
much as OCC and classic transactions, so the second week focuses on it.

**Assumptions:** about 2–3 focused hours a day. "Skim" means only the first two passes of
Keshav's three-pass method (below).

> These citations were written from memory, so check the venue and year before citing any of them.

## Core references (used throughout)

- [ ] **Kleppmann, *Designing Data-Intensive Applications*** (DDIA). Chapter numbers below are
  from the 1st edition. If you have the 2nd edition (with Riccomini), look up the same topics there.
- [ ] **Kleppmann's Cambridge *Distributed Systems* lecture notes and videos.** They're free,
  short, and cover the clock and CRDT material especially well.
- [ ] **CMU 15-445 lectures on concurrency control** (on YouTube). Andy Pavlo's 2PL, timestamp
  ordering/OCC and MVCC lectures are the fastest way into OCC.
- [ ] **Keshav, "How to Read a Paper"** (3 pages). Read it first; it's how you'll get through
  about 20 papers in 14 days.

## Week 1: Transactions, replication, consensus

### Day 1: Distributed systems basics

- [ ] DDIA ch. 8 (The Trouble with Distributed Systems) and ch. 5 (Replication)
- [ ] Lamport, "Time, Clocks, and the Ordering of Events in a Distributed System" (CACM 1978)

*Takeaway:* partial failure, no global clock, the happens-before relation, and leader vs.
multi-leader vs. leaderless replication.

### Day 2: Isolation

- [ ] DDIA ch. 7 (Transactions)
- [ ] Berenson et al., "A Critique of ANSI SQL Isolation Levels" (SIGMOD 1995)
- [ ] The consistency-models map at <https://jepsen.io/consistency>

*Takeaway:* lost updates, write skew and phantoms, and why snapshot isolation is not serializable.

### Day 3: OCC itself

- [ ] Kung & Robinson, "On Optimistic Methods for Concurrency Control" (TODS 1981)
- [ ] The CMU 15-445 lectures on 2PL, timestamp ordering/OCC and MVCC

*Takeaway:* OCC's read, validate and write phases, backward vs. forward validation, and when OCC
beats locking (low contention).

### Day 4: Modern OCC/MVCC

- [ ] Tu et al., "Speedy Transactions in Multicore In-Memory Databases" (Silo, SOSP 2013)
- [ ] Wu et al., "An Empirical Evaluation of In-Memory Multi-Version Concurrency Control" (VLDB 2017)

*Takeaway:* epoch-based OCC with no central timestamp bottleneck, and the main MVCC design choices
(version storage, garbage collection, indexes).

### Day 5: Consensus

- [ ] DDIA ch. 9 (Consistency and Consensus)
- [ ] Ongaro & Ousterhout, "In Search of an Understandable Consensus Algorithm" (Raft, ATC 2014),
  plus the visualization at <https://thesecretlivesofdata.com/raft/>

*Takeaway:* linearizable vs. causal consistency, and the fact that consensus is equivalent to
total-order broadcast. You'll need it for metadata such as membership and who owns which region,
even if not for scene data.

### Day 6: Distributed transactions

- [ ] Peng & Dabek, "Large-scale Incremental Processing Using Distributed Transactions and
  Notifications" (Percolator, OSDI 2010)
- [ ] Corbett et al., "Spanner: Google's Globally-Distributed Database" (OSDI 2012), sections 1–4

*Takeaway:* two-phase commit layered on a key-value store, snapshot isolation using timestamps,
and TrueTime with commit-wait.

### Day 7: Partitioning and distributed OCC (lighter day, leaves room to catch up)

- [ ] DDIA ch. 6 (Partitioning)
- [ ] Dragojević et al., "No compromises: distributed transactions with consistency,
  availability, and performance" (FaRM, SOSP 2015). Read the commit protocol and skip the RDMA
  details.

*Takeaway:* OCC with validation spread across partitions, which is the canonical design for
distributed OCC.

## Week 2: Conflict resolution, graph databases, your domain

### Day 8: Application-level conflict resolution

- [ ] DeCandia et al., "Dynamo: Amazon's Highly Available Key-value Store" (SOSP 2007)
- [ ] Terry et al., "Managing Update Conflicts in Bayou, a Weakly Connected Replicated Storage
  System" (SOSP 1995)

*Takeaway:* version vectors and keeping conflicting versions side by side. Bayou's dependency
checks and merge procedures were designed for weakly connected clients, which fits robots well.

### Day 9: CRDTs

- [ ] Shapiro et al., "Conflict-free Replicated Data Types" (SSS 2011)
- [ ] The CRDT lecture in Kleppmann's notes
- [ ] Skim: Kleppmann et al., "Local-first software" (Onward! 2019)

*Takeaway:* state-based vs. operation-based CRDTs, merging as a semilattice join, and the
pitfalls of add-wins sets and last-writer-wins registers.

### Day 10: Replicated trees and time

- [ ] Kleppmann et al., "A Highly-Available Move Operation for Replicated Trees" (IEEE TPDS 2022).
  This is the closest paper to Yggdrasil's `nest(x).under(y)`: two concurrent moves can create a
  cycle.
- [ ] Skim: Kulkarni et al., "Logical Physical Clocks and Consistent Snapshots in Globally
  Distributed Databases" (Hybrid Logical Clocks, OPODIS 2014). These timestamps respect causality
  and stay close to wall-clock time, which helps with sensor-stamped updates.

### Day 11: When you need coordination at all

- [ ] Bailis et al., "Coordination Avoidance in Database Systems" (VLDB 2015)
- [ ] Hellerstein & Alvaro, "Keeping CALM: When Distributed Consistency is Easy" (CACM 2020)

*Takeaway:* invariant confluence, i.e. which constraints still hold after merging concurrent
updates. Test it against your layer DAG's acyclicity and nesting rules.

### Day 12: Graph database fundamentals

- [ ] Robinson, Webber & Eifrem, *Graph Databases* (free from Neo4j), ch. 6 (Graph Database
  Internals)
- [ ] Besta et al., "Demystifying Graph Databases: Analysis and Taxonomy of Data Organization,
  System Designs, and Graph Queries" (ACM CSUR 2023). Read the sections on data organization,
  distribution and transactions, and skim the rest.

*Takeaway:* index-free adjacency vs. graphs stored on a key-value or relational backend, the
property-graph model, and how storage layout affects traversal speed.

### Day 13: Distributed graph stores

- [ ] Bronson et al., "TAO: Facebook's Distributed Data Store for the Social Graph" (ATC 2013)
- [ ] Dubey et al., "Weaver: A High-Performance, Transactional Graph Database Based on Refinable
  Timestamps" (VLDB 2016)
- [ ] Skim: Jepsen's Dgraph analyses

*Takeaway:* TAO is read-optimized and eventually consistent. Weaver offers serializable graph
transactions using refinable timestamps. The Jepsen reports show what actually breaks in practice.

### Day 14: Your domain, then put it together

- [ ] Chang et al., "Hydra-Multi: Collaborative Online Construction of 3D Scene Graphs with
  Multi-Robot Teams" (IROS 2023). This is how the robotics community currently merges scene
  graphs from several robots.
- [ ] Skim: Stanton & Kliot, "Streaming Graph Partitioning for Large Distributed Graphs"
  (KDD 2012), for edge-cut vs. vertex-cut partitioning.
- [ ] Write a short design memo answering the questions below.

## Questions to answer by day 14

1. **Unit of replication and ownership:** the whole graph, a layer, a spatial tile or a subtree?
   Spatial queries suggest partitioning by space rather than with a generic graph partitioner.
2. **Consistency for queries:** does a field-of-view or nearest-neighbor query need linearizable
   reads, or is causal consistency or bounded staleness enough?
3. **Which operations commute and which need coordination:** feature edits and point appends can
   probably be CRDTs. `nest`/move under layer-DAG constraints and creating layers probably can't.
4. **Where OCC fits and where it fails:** it suits low-contention, multi-node edits against a
   reachable server. A disconnected robot can't run the validation step, which is where Bayou and
   CRDT approaches take over.
5. **Derived state:** should bounding boxes and position indexes be replicated, or rebuilt
   locally from the replicated base operations? DDIA ch. 11–12 on event sourcing argues for
   rebuilding.
6. **What counts as a conflict:** when two robots see the same car, is that a conflict to resolve
   or a data-association problem? Compare Hydra-Multi with Bayou's merge procedures.

## Optional extras if a day goes fast

- [ ] Bernstein, Hadzilacos & Goodman, *Concurrency Control and Recovery in Database Systems*
  (free from Microsoft Research): the reference for serializability theory.
- [ ] Adya, "Weak Consistency: A Generalized Theory and Optimistic Implementations for
  Distributed Transactions" (PhD thesis, MIT 1999): the formal theory behind the Jepsen map.
- [ ] Yu et al., "TicToc: Time Traveling Optimistic Concurrency Control" (SIGMOD 2016) and Yu et
  al., "Staring into the Abyss: An Evaluation of Concurrency Control with One Thousand Cores"
  (VLDB 2014): more OCC variants and how they scale.
- [ ] Thomson et al., "Calvin: Fast Distributed Transactions for Partitioned Database Systems"
  (SIGMOD 2012) and Abadi & Faleiro, "An Overview of Deterministic Database Systems" (CACM 2018):
  agree on an order first and skip two-phase commit.
- [ ] Buragohain et al., "A1: A Distributed In-Memory Graph Database" (SIGMOD 2020), a graph
  database built on FaRM.
- [ ] Zhu et al., "LiveGraph: A Transactional Graph Storage System with Purely Sequential
  Adjacency List Scans" (VLDB 2020): MVCC storage designed for adjacency lists.
- [ ] Bailis et al., "Highly Available Transactions: Virtues and Limitations" (VLDB 2014).
- [ ] Helland, "Life beyond Distributed Transactions: an Apostate's Opinion" (CIDR 2007).
- [ ] Guttman, "R-trees: A Dynamic Index Structure for Spatial Searching" (SIGMOD 1984), if you
  decide to partition by space.
- [ ] MIT 6.5840's Raft labs. They're the best way to learn consensus by building it, but they
  won't fit in these two weeks alongside everything else.
