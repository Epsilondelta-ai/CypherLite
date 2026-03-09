# CypherLite Storage Engine Design

**Document Date**: March 10, 2026
**Purpose**: Complete specification of CypherLite's storage engine, file format, and storage architecture.
**Status**: Design Phase
**Version**: 1.0

---

## Table of Contents

1. [Overview](#overview)
2. [File Format Specification](#file-format-specification)
3. [Page Types and Structures](#page-types-and-structures)
4. [Node Storage](#node-storage)
5. [Edge/Relationship Storage](#edgerelationship-storage)
6. [Property Storage](#property-storage)
7. [Index Structures](#index-structures)
8. [Free Space Management](#free-space-management)
9. [Write-Ahead Log (WAL)](#write-ahead-log-wal)
10. [Temporal Storage Extension](#temporal-storage-extension)
11. [Recovery and Durability](#recovery-and-durability)

---

## Overview

CypherLite adopts a **single-file, page-based storage engine** combining SQLite's simplicity with Neo4j's graph-native design. The storage engine is organized around the following principles:

- **Single-file paradigm**: All data, indices, and metadata in one `.cyl` file
- **Page-based architecture**: 4KB pages (configurable 512B to 64KB) for cache efficiency
- **Index-free adjacency**: Direct pointers between nodes and relationships for O(1) traversal
- **Separation of concerns**: Distinct page types for nodes, edges, properties, and indices
- **WAL transactions**: Write-ahead log for ACID guarantees and concurrent readers
- **Temporal support**: Optional temporal versioning of nodes and edges

### Design Goals

1. **Simplicity**: Single-file deployment and simple data structure navigation
2. **Performance**: Index-free adjacency, B-tree indices, memory mapping support
3. **Concurrency**: WAL mode for multiple concurrent readers with single writer
4. **Embedded focus**: Minimal dependencies, resource-constrained optimizations
5. **Extensibility**: Room for temporal, RDF, and full-text search capabilities

---

## File Format Specification

### File Header Layout

The CypherLite file begins with a 512-byte header containing metadata about the database:

```
CypherLite Database File Header (512 bytes)
============================================

Offset  Size  Field                    Description
------  ----  -----                    -----------
0       16    magic_bytes              "CYPHERLITE\0" + 5 padding bytes
16      4     format_version           0x00010000 (v1.0.0)
20      4     page_size                Default 4096 (512-65536 supported)
24      8     file_create_time         Unix timestamp (seconds since epoch)
32      8     last_checkpoint_time     Time of last WAL checkpoint
40      8     total_pages              Total number of pages in file
48      4     flags                    Database configuration flags
                  Bit 0: WAL enabled (1) or rollback (0)
                  Bit 1: Compression enabled
                  Bit 2: Temporal versioning enabled
                  Bit 3: Full-text indexing enabled
                  Bits 4-31: Reserved
52      4     default_isolation        0=SERIALIZABLE, 1=SNAPSHOT, 2=READ_UNCOMMITTED
56      8     node_count               Current number of nodes
64      8     edge_count               Current number of edges
72      8     label_table_root_page    Page number of label lookup table
80      8     type_table_root_page     Page number of relationship type table
88      8     property_key_root_page   Page number of property key dictionary
96      8     free_page_list_page      Page number of free page list head
104     8     label_index_root_page    B-tree root page for label indices
112     8     type_index_root_page     B-tree root page for type indices
120     8     property_index_root_page B-tree root page for property indices
128     8     node_store_root_page     B-tree root page for node store
136     8     edge_store_root_page     B-tree root page for edge store
144     8     property_store_root_page B-tree root page for property store
152     8     temporal_store_root      B-tree root page for temporal data (if enabled)
160     4     max_label_id             Highest assigned label ID
164     4     max_type_id              Highest assigned relationship type ID
168     4     max_property_key_id      Highest assigned property key ID
172     4     transaction_id           Current transaction counter
176     4     wal_checkpoint_count     Number of WAL checkpoints performed
180     16    reserved_1               Reserved for future use
196     8     wal_frame_count          Number of frames in active WAL
204     8     wal_frame_offset         Byte offset of current WAL file
212     300   reserved_2               Reserved for future extensions

512 bytes total
```

**Magic Bytes Explanation**:
- `0x43 0x59 0x50 0x48 0x45 0x52 0x4C 0x49 0x54 0x45` = "CYPHERLITE" (ASCII)
- Followed by null terminator + padding

### Page Structure (General)

All pages (except header) follow this structure:

```
Generic Page Structure (4KB default)
====================================

Offset  Size  Field                    Description
------  ----  -----                    -----------
0       4     page_number              Page ID within file
4       1     page_type                0x01=Header, 0x02=Node, 0x03=Edge,
                                       0x04=Property, 0x05=Index, 0x06=Free,
                                       0x07=Temporal, 0x08=LabelTable,
                                       0x09=TypeTable, 0x0A=PropertyKeyTable
5       1     page_flags               Bit 0: Dirty, Bit 1: Index interior,
                                       Bit 2: Index leaf, Bits 3-7: Reserved
6       2     free_space_offset        Byte offset where free space starts
8       4     record_count             Number of records in this page
12      2     slot_array_offset        Byte offset of first slot in slot array
14      2     slot_array_size          Total size of slot array (in slots)
16      8     checksum                 CRC-64 of page content (0 during compute)
24      8     page_lsn                 Log Sequence Number from WAL

32 bytes: Page Header
32-4064: Slot Array (variable size, typically 1-2KB)
         Each slot: [2 bytes offset, 2 bytes size]
         Growing downward from slot_array_offset

Variable: Free Space (working area for records)

4064-4096: Trailer
           [Previous page number (4 bytes)]
           [Next page number (4 bytes)]
           [Timestamp (4 bytes)]
           [Reserved (16 bytes)]
           Total: 32 bytes

4096 bytes total
```

### File Growth Strategy

CypherLite grows the file incrementally:

1. **Initial creation**: File starts at minimum size (512-byte header + 1 metadata page)
2. **On-demand allocation**: When free space exhausted, allocate new pages
3. **Batch allocation**: Allocate in chunks (e.g., 10 pages at a time) to reduce OS calls
4. **Free list tracking**: Free Page List pages track available pages for reuse
5. **Vacuum/Compaction**: Optional background process to reclaim fragmented pages

**Page Allocation Algorithm**:
- Primary: Reuse pages from Free Page List
- Secondary: Allocate at end of file
- Fallback: Reuse fragmented pages if space available

**File Size Growth**:
- Start: 512 bytes (header) + 4096 bytes (first metadata page) = 4608 bytes
- Typical chunk: +40960 bytes (10 pages at a time)
- Max tested: 16TB single file (demonstrates extreme scalability)

---

## Page Types and Structures

### 1. Header Page (Type 0x01)

Contains critical database metadata. Pages 0-1 are reserved for dual copies of header (similar to SQLite). Only one header page at offset 0.

### 2. Node Pages (Type 0x02)

Stores node records in a B-tree structure. Node records are fixed-size to enable O(1) lookup by ID:

```
Node Page Structure
===================

Header (32 bytes)
  - Standard page header

Slot Array (variable)
  - Array of [offset: 2B, size: 2B] pairs
  - One slot per node record
  - Slots grow downward from slot_array_offset

Data Area
  - Node records (packed after slot array)
  - Variable-length layout
```

### 3. Edge Pages (Type 0x03)

Stores relationship records. Maintains doubly-linked adjacency lists at the byte level.

### 4. Property Pages (Type 0x04)

Stores property key-value pairs. Large properties overflow to dedicated property value pages.

### 5. Index Pages (Type 0x05)

B+-tree pages for label indices, type indices, and property indices. Index interior pages contain keys and child pointers; leaf pages contain keys and record IDs.

### 6. Free Pages (Type 0x06)

Track available pages for allocation. Organized as a linked list of Free Page List pages.

### 7. Temporal Pages (Type 0x07)

Store temporal versions of nodes and edges (if temporal versioning enabled). Uses anchor + delta storage.

### 8. Label/Type/PropertyKey Tables (Types 0x08, 0x09, 0x0A)

Dictionary tables mapping labels/types/keys to numeric IDs. Similar structure to dictionary compression.

---

## Node Storage

### Node Record Format

Nodes are stored with a hybrid fixed/variable-size layout to balance lookup speed with flexibility:

```
Node Record Structure
=====================

Fixed Part (64 bytes):
  Offset  Size  Field                    Description
  ------  ----  -----                    -----------
  0       8     node_id                  Unique node identifier (u64)
  8       4     label_count              Number of labels for this node
  12      4     label_bitmap_offset      Byte offset to label bitmap (relative to record start)
  16      8     first_edge_ptr           Page:Slot pointer to first outgoing edge
                                         Upper 32b: page, lower 32b: slot
  24      8     first_property_ptr       Page:Slot pointer to first property record
  32      8     temporal_version_ptr     Page:Slot pointer to temporal anchor (if versioning enabled)
  40      4     flags                    Record flags
                   Bit 0: Has properties
                   Bit 1: Has temporal versions
                   Bit 2: Is deleted (soft delete)
                   Bits 3-31: Reserved
  44      4     create_timestamp         Creation time (seconds since epoch)
  48      4     modify_timestamp         Last modification time
  52      4     property_count           Total number of properties
  56      8     record_size              Total size of this record in bytes

Variable Part:
  [Label Bitmap]           Variable, 1-64 bytes
                          Bitmap of label IDs assigned to this node
  [Inline Properties]      Variable, 0-512 bytes
                          Small properties stored inline
  [Reserved/Padding]       Variable
                          Align to 8-byte boundary
```

**Total node record size**: 64 + variable (typical 100-500 bytes for average node)

### Label Storage Strategy

Labels are stored in three ways:

1. **Label Bitmap**: Each node has a bitmap indicating which labels apply
   - Node's `label_bitmap_offset` points to start of bitmap
   - Bitmap is sparse (only non-zero label ranges included)
   - Example: If node has labels {1, 5, 17}, bitmap encodes their positions

2. **Label Table**: Global label ID → label name mapping
   - Located at page referenced by header's `label_table_root_page`
   - Format: [label_id: 4B, name_len: 2B, name: variable]
   - Enables efficient label lookups

3. **Label Index**: B+-tree from label ID → [list of node IDs]
   - Root page at `label_index_root_page`
   - Enables fast queries like "MATCH (n:User)"
   - Leaf nodes contain node IDs with that label

### Fixed-Size vs Variable-Size Parts

**Fixed Part (64 bytes)**:
- Always present at record start
- Enables O(1) location calculation: `offset = page_number * page_size + slot_offset`
- Contains pointers to variable data

**Variable Part**:
- Label bitmap (1-64 bytes typical)
- Inline properties (0-512 bytes for small values)
- Large properties overflow to property pages

This hybrid approach enables:
- Fast direct access by node ID
- Flexible storage for variable label counts
- Optimization for small properties
- Graceful handling of large properties

### Node ID to Page Mapping

Node IDs are mapped to physical locations via B-tree:

```
Node Lookup Algorithm
====================

1. Start at node_store_root_page (from file header)
2. B-tree navigation:
   - Interior pages: Binary search for key range
   - Leaf pages: Direct slot lookup
3. Retrieve node record via slot array
4. Access time: O(log n) where n = node count
```

---

## Edge/Relationship Storage

### Edge Record Format

Edges are stored with full pointers to enable index-free adjacency:

```
Edge Record Structure
=====================

Fixed Part (96 bytes):
  Offset  Size  Field                         Description
  ------  ----  -----                         -----------
  0       8     edge_id                       Unique edge identifier (u64)
  8       4     type_id                       Relationship type ID
  12      4     source_node_id                Start node ID (u32 or u64 depending on config)
  16      8     target_node_id                End node ID
  24      8     next_outgoing_edge_ptr        Page:Slot of next edge from source node
                                              Maintains adjacency chain at source
  32      8     prev_outgoing_edge_ptr        Page:Slot of previous edge from source node
                                              Bidirectional chain links
  40      8     next_incoming_edge_ptr        Page:Slot of next edge to target node
                                              Maintains adjacency chain at target
  48      8     prev_incoming_edge_ptr        Page:Slot of previous edge to target node
  56      8     first_property_ptr            Page:Slot pointer to first property record
  64      8     temporal_version_ptr          Page:Slot pointer to temporal anchor (if enabled)
  72      4     flags                         Record flags
                   Bit 0: Has properties
                   Bit 1: Has temporal versions
                   Bit 2: Is deleted (soft delete)
                   Bits 3-31: Reserved
  76      4     create_timestamp              Creation time
  80      4     modify_timestamp              Last modification time
  84      4     property_count                Total number of properties
  88      8     record_size                   Total size of record

Variable Part:
  [Inline Properties]      Variable, 0-512 bytes
  [Reserved/Padding]       Variable for alignment
```

**Total edge record size**: 96 + variable (typical 120-600 bytes)

### Doubly-Linked Adjacency Lists

The four pointer fields (`next_outgoing`, `prev_outgoing`, `next_incoming`, `prev_incoming`) implement doubly-linked adjacency lists:

```
Example: Node A has 3 outgoing edges to B, C, D

Node A Record:
  first_edge_ptr → Edge1 (to B)

Edge1 (A→B):
  next_outgoing_ptr → Edge2
  prev_outgoing_ptr → null (first edge)

Edge2 (A→C):
  next_outgoing_ptr → Edge3
  prev_outgoing_ptr → Edge1

Edge3 (A→D):
  next_outgoing_ptr → null (last edge)
  prev_outgoing_ptr → Edge2

Traversal: Start at node.first_edge_ptr, follow next pointers
Time: O(degree) to traverse all edges from node
```

**Benefits of doubly-linked structure**:
- Efficient insertion/deletion in adjacency lists
- Bidirectional traversal (forward and backward)
- Constant-time edge lookup (given edge ID)

### Type Storage

Relationship types stored similarly to labels:

1. **Type Table**: Global type ID → type name mapping
   - At page `type_table_root_page`
   - Format: [type_id: 4B, name_len: 2B, name: variable]

2. **Type Index**: B+-tree from type ID → [list of edge IDs]
   - Root at `type_index_root_page`
   - Enables "MATCH ()-[:FOLLOWS]->()" queries efficiently

---

## Property Storage

### Property Record Format

Properties are stored as key-value pairs with support for multiple data types:

```
Property Record Structure
=========================

Fixed Part (48 bytes):
  Offset  Size  Field                    Description
  ------  ----  -----                    -----------
  0       8     property_id              Unique property ID (u64)
  8       4     key_id                   Property key ID (references property key table)
  12      1     value_type               Type of value:
                   0x00 = null
                   0x01 = bool
                   0x02 = int8, 0x03 = int16, 0x04 = int32, 0x05 = int64
                   0x06 = uint8, 0x07 = uint16, 0x08 = uint32, 0x09 = uint64
                   0x0A = float32, 0x0B = float64
                   0x0C = string (inline), 0x0D = string (overflow)
                   0x0E = bytes (inline), 0x0F = bytes (overflow)
                   0x10 = list, 0x11 = map
                   0x12 = datetime, 0x13 = date, 0x14 = time, 0x15 = duration
                   0x16 = point (spatial)
  13      1     flags                    Bit 0: Is overflow
                                         Bit 1: Is compressed
                                         Bits 2-7: Reserved
  14      2     inline_size              Size of inline value (0 if overflow)
  16      4     overflow_page_ptr        Page number if value overflows (otherwise 0)
  20      4     overflow_slot            Slot in overflow page (otherwise 0)
  24      8     next_property_ptr        Page:Slot of next property in chain
  32      8     parent_entity_ptr        Page:Slot of owning node/edge
  40      4     value_length             Full length of value (for inline, same as inline_size)
  44      4     compression_ratio        Compression ratio if compressed (0 if not)

Inline Value Part (variable, 0-256 bytes):
  Raw value data for inline storage
```

### Value Types and Inline Thresholds

```
Type           Storage                  Inline Threshold
----           -------                  ----------------
null           N/A (0 bytes)            Always inline
bool           1 byte                   Always inline
int64          8 bytes                  Always inline
float64        8 bytes                  Always inline
datetime       12 bytes (seconds + nanos) Always inline
date           4 bytes                  Always inline
time           8 bytes                  Always inline
point/spatial  24-32 bytes              Always inline
string         Variable length          256 bytes
bytes          Variable length          256 bytes
list           Variable length          512 bytes (compressed)
map            Variable length          512 bytes (compressed)
```

### Overflow Property Storage

Large properties overflow to Property Value Pages:

```
Property Value Page (for large values)
======================================

Header (32 bytes): Standard page header

Data Area:
  [Overflow Value 1]     Variable length
  [Overflow Value 2]     Variable length
  ...
  [Free Space]

Slot Array (at page bottom, growing upward)
  Each slot: [offset: 2B, size: 2B]
  One slot per overflow value

Max single value: (page_size - 32) bytes
For multi-page values: Linked list of overflow pages
```

### Property Key Dictionary

Property keys stored in global dictionary:

```
Property Key Table
==================

Format: Sparse B-tree with entries
  [key_id: 4B, name_len: 2B, name: variable]
  Stored in order of key_id for binary search

Example:
  [0, 4, "name"]
  [1, 3, "age"]
  [2, 5, "email"]
  [3, 7, "address"]
```

---

## Index Structures

### B+ Tree Index Organization

All indices use B+ trees for efficient range queries and sorted access:

**Interior Node Page** (B+ tree):
```
B+ Tree Interior Page
====================

Standard Page Header (32 bytes)

Content:
  [Key 1: 8B] [Child Page Ptr 1: 4B]
  [Key 2: 8B] [Child Page Ptr 2: 4B]
  ...
  [Key N: 8B] [Child Page Ptr N: 4B]

Keys are sorted; pointers guide navigation
```

**Leaf Node Page** (B+ tree):
```
B+ Tree Leaf Page
=================

Standard Page Header (32 bytes)

Content:
  [Key 1: 8B] [Value 1: variable]
  [Key 2: 8B] [Value 2: variable]
  ...

Values stored inline for small data (< 256 bytes)
For larger values: [Pointer to overflow page]

Leaf pages linked: [prev_leaf_ptr] ... [next_leaf_ptr]
Enables full tree scans efficiently
```

### Label Index (B+ Tree)

Maps label ID → [list of node IDs]

```
Label Index B+ Tree
===================

Key: label_id (u32)
Value: Compressed node ID list or pointer to node list page

Root at: file_header.label_index_root_page

Example tree for labels {1, 5, 17}:
  Interior nodes route by label_id ranges
  Leaf nodes contain [label_id, [node_id1, node_id2, ...]]
  Node ID lists stored inline if < 256 bytes, else overflow to separate pages

Query: "MATCH (n:User)"
  1. Lookup "User" in label table → label_id = 5
  2. Search label index tree for key=5
  3. Retrieve node ID list from leaf
  4. Fetch nodes by ID from node store
```

### Type Index (B+ Tree)

Maps relationship type ID → [list of edge IDs]

```
Type Index B+ Tree
==================

Key: type_id (u32)
Value: Compressed edge ID list or pointer to edge list page

Root at: file_header.type_index_root_page

Similar structure to label index
Enables efficient "MATCH ()-[:FOLLOWS]->()" queries
```

### Property Index (B+ Tree)

Maps (property_key_id, property_value) → [list of entity IDs]

```
Property Index B+ Tree (Multi-dimensional)
===========================================

Composite Key: (key_id: 4B, value: variable)
Value: [entity_id list]

Organization:
  First level keyed by property_key_id
  Second level keyed by property_value (within each key)

Root at: file_header.property_index_root_page

Example: Index on (name, age)
  Index query "MATCH (n) WHERE n.age > 21"
    1. Find property index for property_key_id = "age"
    2. Range scan B+ tree for values > 21
    3. Collect all node IDs from results

Composite index support:
  Create separate indices for frequently used property combinations
  Example: (age, city) composite index for (n.age > 21 AND n.city = 'NYC')
```

### Full-Text Index (Optional)

If bit 3 of flags in header set, support full-text search:

```
Full-Text Index Structure
=========================

Components:
  1. Token Inverted Index: B+ tree mapping token → (doc_id, position_list)
  2. Token Dictionary: Mapping of normalized tokens to token_ids
  3. Document Store: Original text values for display
  4. Vector Index (optional): Vector embeddings for semantic search

Root page: Custom location based on implementation
Typically uses separate page trees like main indices
```

---

## Free Space Management

### Free Page List

CypherLite maintains a linked list of free pages:

```
Free Page List Head Page
========================

Standard Page Header (32 bytes)

Content:
  [free_page_count: 4B]
  [next_free_page_list: 4B]  (link to next Free Page List page if overflowed)

  [free_page_1: 4B]
  [free_page_2: 4B]
  ...
  [free_page_N: 4B]

  Each free page number is 4 bytes (u32)
  Max free pages per list page: (page_size - 40) / 4
  For 4KB pages: ~1024 free pages per list page

When list overflows:
  Create new Free Page List page
  Link via next_free_page_list pointer
  Forms chain of Free Page List pages
```

**Free Page Allocation Algorithm**:
```
Algorithm: Allocate Free Page
==============================

1. Load Free Page List head page from header
2. If free_page_count > 0:
     a. Remove last entry from list
     b. Decrement free_page_count
     c. Return freed page number
3. Else:
     a. Allocate at end of file
     b. Update header.total_pages
     c. Return new page number
4. Mark page with type 0x00 (unallocated)
```

### Free Space Within Pages

Each page maintains free space tracking:

```
Within-Page Free Space Management
==================================

For variable-length pages (Node, Edge, Property pages):

1. slot_array grows downward from page bottom
2. Records grow upward from page start
3. Free space is gap between highest record and lowest slot

Free space allocation within page:
  - First fit: Find first slot large enough
  - Best fit: Find slot closest to exact size
  - Pack small records to reduce fragmentation

Compaction strategy:
  When free space becomes too fragmented (> 30% wasted):
    1. Copy live records to temp buffer
    2. Clear all slots
    3. Rewrite records contiguously
    4. Update slot array
    5. Consolidate free space at end
```

### Page Compaction and Vacuum

**Automatic Compaction**:
- Triggered when page waste exceeds 30%
- Can run online without blocking reads (copy-on-write)

**VACUUM Command**:
- Explicit full database compaction
- Rewrite entire page sequences to eliminate fragmentation
- Create new contiguous file
- Atomically swap files (or copy back)

```
Vacuum Algorithm
================

1. Open database file R (read-only)
2. Create new temporary database file W (write)
3. For each page P in R:
     a. If P is used (not in free list):
        - Compacting page (remove unused slots)
        - Write compacted P to W
        - Update internal pointers
     b. If P is free:
        - Skip (don't write)
4. Update header with new page count
5. Close R, replace original with W
6. Commit on success (atomic via filesystem operations)
```

---

## Write-Ahead Log (WAL)

CypherLite implements SQLite-style WAL for ACID transactions and concurrent readers.

### WAL File Format

WAL file (`.cyl-wal`) stores frames describing page modifications:

```
WAL File Header (32 bytes)
==========================

Offset  Size  Field                    Description
------  ----  -----                    -----------
0       4     magic                    0x377f0682 (little-endian) or
                                       0x377f0683 (big-endian)
4       4     version                  0x3007000 (v3, release 7)
8       4     page_size                Page size (must match main DB)
12      4     checkpoint_seq           Checkpoint sequence number
16      4     salt_1                   Random salt for WAL validation
20      4     salt_2                   Random salt for WAL validation
24      4     frame_count              Number of frames in this WAL
28      4     checksum                 CRC32 of header

32 bytes total
```

### WAL Frame Structure

Each frame represents one page modification:

```
WAL Frame Structure (one per modified page)
============================================

Frame Header (24 bytes):
  Offset  Size  Field                  Description
  ------  ----  -----                  -----------
  0       4     page_number            Which page modified (u32)
  4       4     frame_size             Size of payload (usually page_size)
  8       4     commit_marker          If set, transaction committed after this frame
  12      4     salt_checksum          Copy of header salt for validation
  16      4     frame_checksum         CRC32 of [frame header + payload]
  20      4     frame_number           Sequential frame number (1, 2, 3, ...)

Frame Payload:
  page_size bytes: Complete modified page content

Total frame size: 24 + page_size bytes
(e.g., 24 + 4096 = 4120 bytes for 4KB pages)

Multiple frames per transaction:
  Frame 1: page_num=5, payload=[modified node page 5]
  Frame 2: page_num=10, payload=[modified edge page 10]
  Frame 3: page_num=15, commit_marker=1, payload=[modified property page 15]
  Commit happens after frame 3
```

### WAL Index File (Shared Memory)

CypherLite optionally uses a `.cyl-shm` file for in-memory WAL index (similar to SQLite):

```
WAL Index Structure (.cyl-shm)
===============================

Supports fast lookup of which pages are modified in WAL
Speeds up readers determining if they need to check WAL

Structure:
  [WAL version]
  [Lock region for concurrency]
  [Hash table mapping page_number → frame_index]
  [Frame index list]
  [Commit points]

Allows readers to quickly determine:
  "Is page N in WAL?" → Hash lookup
  "What's the latest version of page N?" → Follow hash to frame
```

### Checkpoint Algorithm

Checkpointing transfers WAL frames back to main database:

```
Checkpoint Process
==================

PASSIVE checkpoint (default):
  1. Acquire write lock on database
  2. For each frame F in WAL:
     a. Read frame F payload
     b. Write to main database file at page_offset
     c. Update checkpoint_seq
  3. Truncate WAL file (or recycle)
  4. Release write lock
  5. Readers can now read from main DB without WAL

Time: O(WAL size), roughly WAL size / page_size

RESTART checkpoint (on explicit PRAGMA wal_checkpoint):
  Same as PASSIVE, but:
  - Delete WAL and WAL index completely
  - Reset frame counter to 0
  - Most aggressive checkpoint

FULL checkpoint:
  Same as RESTART, but:
  - Also run page-level compaction
  - Defragment database file
  - Most resource-intensive but best file layout

Automatic checkpoint triggers:
  - When WAL reaches size threshold (e.g., 1000 pages)
  - On database close
  - On explicit PRAGMA command
```

### Recovery Procedure

On startup, CypherLite checks for incomplete WAL:

```
Recovery Algorithm
==================

On Database Open:
  1. Read main database header
  2. Check for .cyl-wal file existence
  3. If WAL exists:
     a. Read WAL header, validate magic bytes
     b. Check salt_1/salt_2 against main header
     c. If salts don't match: WAL is stale, delete it
     d. If valid:
        - Iterate through WAL frames
        - Replay each frame to temporary buffer or direct write
        - For each frame with commit_marker=1, mark transaction complete
        - Handle partial final transaction (if crash):
          * WAL frames without commit_marker: discard (rollback)
          * Frames with commit_marker: persist
     e. After replay, delete or reset WAL file
  4. Database is now in consistent state

Crash scenarios:
  A. Crash during WAL write:
     - Partial frame detected by checksum
     - Discard incomplete frame and stop replay
     - Previous committed frames already in main DB

  B. Crash during checkpoint:
     - WAL frames already written to main DB
     - Some frames may still be in WAL
     - Safe: Checkpoint idempotent (writing same data again is safe)
     - Replay remaining frames to complete checkpoint

  C. Crash during main DB write:
     - Changes not yet in main DB
     - WAL frames still present with change
     - Replay recovers the transaction
```

### Transaction Isolation Levels

CypherLite supports SQLite-style isolation:

```
Isolation Levels
================

SERIALIZABLE (Default):
  - Transaction's changes invisible until commit
  - Other transactions don't see uncommitted changes
  - Implementation: WAL frame's commit_marker gates visibility
  - Readers use snapshot of main DB + committed WAL frames

SNAPSHOT (WAL Mode):
  - Readers see consistent snapshot from transaction start
  - Readers don't block writers, writers don't block readers
  - Implementation: Each reader snapshots main DB + WAL frame list at start
  - Writer adds frames, readers continue with original snapshot
  - On reader finish: Fetch any new committed frames for next query

READ_UNCOMMITTED (Dirty Reads):
  - Readers may see uncommitted frames from current WAL
  - Fastest but least safe
  - Implementation: Readers include all WAL frames, even uncommitted
```

---

## Temporal Storage Extension

If temporal versioning enabled (header bit 2), CypherLite supports temporal queries:

### Temporal Model

```
Temporal Versioning Model
=========================

Each node/edge can have multiple versions with different validity periods:

Node v1 (time [2024-01-01, 2024-06-01]):
  Properties: {name: "Alice", age: 30}

Node v2 (time [2024-06-01, NULL]):
  Properties: {name: "Alice", age: 31}

Query: "MATCH (n) AS OF 2024-03-01"
  Returns version v1 (valid at that time)

Query: "MATCH (n) BETWEEN 2024-01-01 AND 2024-12-31"
  Returns both versions (valid within range)
```

### Temporal Record Structure

```
Temporal Anchor (stored in edge records)
========================================

Fixed Part (48 bytes):
  Offset  Size  Field                    Description
  ------  ----  -----                    -----------
  0       8     anchor_id                Unique ID for this temporal anchor
  8       8     entity_id                ID of node/edge being versioned
  16      8     anchor_version           Base version ID
  24      4     version_count            Number of versions
  28      4     flags                    Bit 0: Current version, Bits 1-31: Reserved
  32      8     first_version_ptr        Page:Slot of first temporal version record
  40      8     time_index_ptr           Page:Slot of temporal index (B-tree by time)

Temporal Version Record:
  Offset  Size  Field                    Description
  ------  ----  -----                    -----------
  0       8     version_id               u64
  8       8     valid_from               Unix timestamp (seconds)
  16      8     valid_until              Unix timestamp (NULL = unbounded)
  24      4     delta_size               Size of delta vs base version
  28      4     next_version_ptr         Page:Slot of next version
  32      Variable delta_data            Compressed delta from anchor_version
```

### Anchor + Delta Approach

To save space, temporal versions use delta encoding:

```
Temporal Storage Example
========================

Base version (anchor):
  name: "Alice", age: 30, email: "alice@example.com"

Version 2 delta (only changed fields):
  [age: 31]  (delta encodes only the change)
  Size: ~20 bytes vs ~100 bytes for full copy
  Compression: 80% space savings

Version 3 delta:
  [age: 32]

Reconstruction algorithm:
  Start with anchor_version properties
  Apply delta_1 changes
  Apply delta_2 changes
  ...
  Result: Properties for requested version

Time-range queries:
  B-tree index by valid_from/valid_until
  Binary search finds versions in range
  O(log n) where n = version count for entity
```

### Temporal Index Structure

Temporal B-tree enables efficient time-range queries:

```
Temporal B+ Tree Index
======================

Key: Composite (valid_from: i64, valid_until: i64)
Value: version_id (u32)

Allows queries:
  "AS OF 2024-06-15" → Binary search for version where
    valid_from <= 2024-06-15 AND (valid_until is NULL OR valid_until > 2024-06-15)

  "BETWEEN 2024-01-01 AND 2024-12-31" → Range scan finding all overlapping versions

Complexity: O(log n + k) where n = versions, k = matching versions
```

### Temporal Query Examples

```cypher
// Query as of specific time
MATCH (n:User) AS OF DATETIME('2024-06-01')
RETURN n.name, n.age

// Query in time range
MATCH (n:User) BETWEEN DATETIME('2024-01-01') AND DATETIME('2024-12-31')
RETURN n.name, n.age, n.__valid_from, n.__valid_until

// Show all versions
MATCH (n:User) ALL VERSIONS
RETURN n.name, n.age, n.__version_id, n.__valid_from
```

---

## Recovery and Durability

### ACID Guarantees

**Atomicity**:
- Transaction either fully committed or fully rolled back
- Implemented via WAL: All frames must have commit_marker or transaction is rolled back

**Consistency**:
- Database maintains constraints and relationships
- Indices kept in sync with data via WAL replay
- Referential integrity checked at transaction level

**Isolation**:
- Transactions don't interfere (SERIALIZABLE default)
- WAL enables SNAPSHOT isolation for readers
- Dirty reads possible with READ_UNCOMMITTED

**Durability**:
- Committed data persists despite crashes
- WAL frames survive crashes
- Recovery replays uncommitted WAL frames
- Main database file survives (pages in main DB are durable immediately)

### Synchronization Pragmas (Configuration)

```
Durability vs Performance Trade-off
===================================

PRAGMA synchronous = FULL (safest):
  After each transaction commit:
    1. fsync() main database file
    2. fsync() WAL file
    3. fsync() directory
  Ensures disk write before returning
  Slowest: ~10-100ms per transaction

PRAGMA synchronous = NORMAL (balanced):
  After each transaction commit:
    1. fsync() main database file
    2. No fsync() WAL (WAL auto-syncs on checkpoint)
  Common scenario: WAL survives most crashes
  Speed: ~1-10ms per transaction
  Risk: WAL frames may be lost in rare OS crashes

PRAGMA synchronous = OFF (fastest):
  No fsync() after commit
  Returns immediately
  Speed: <1ms per transaction
  Risk: Both main DB and WAL may lose committed frames
```

### Backup and Recovery

**Simple Backup**:
```
1. Acquire read lock on main database
2. Copy main .cyl file
3. Copy .cyl-wal file if exists
4. Copy .cyl-shm file if exists
5. Release lock
6. Backup is now a snapshot
```

**Incremental Backup** (WAL-based):
```
1. Note current WAL checkpoint_seq
2. On next checkpoint, copy only new WAL frames since last backup
3. Combine incremental frames with full backup
4. Reduces backup size (delta compression)
```

**Point-in-Time Recovery**:
```
1. Full backup as of date D
2. Replay WAL frames from D forward to desired point in time
3. Requires timestamp metadata in WAL frames (added as optional)
4. Enables recovery to any point within WAL retention period
```

---

## Storage Engine Configuration

### Compile-Time Configuration

```c
// config.h
#define CYPHERLITE_DEFAULT_PAGE_SIZE 4096      // 512 to 65536
#define CYPHERLITE_MAX_NODE_ID 0xFFFFFFFFULL  // u64 max
#define CYPHERLITE_MAX_EDGES_PER_NODE 1000000 // Adjacency list limit
#define CYPHERLITE_INLINE_PROPERTY_SIZE 256    // Bytes before overflow
#define CYPHERLITE_PROPERTY_COMPRESS_THRESHOLD 512  // Compress if larger
#define CYPHERLITE_WAL_CHECKPOINT_SIZE 1000    // Pages before auto-checkpoint
#define CYPHERLITE_CACHE_SIZE_MB 256           // Hot cache size
#define CYPHERLITE_ENABLE_TEMPORAL 1           // 1=enabled, 0=disabled
```

### Runtime Configuration (PRAGMA)

```
PRAGMA page_size = 4096;              // Must set before first use
PRAGMA wal_mode = ON;                 // ON or OFF (rollback journal)
PRAGMA synchronous = NORMAL;          // FULL, NORMAL, OFF
PRAGMA cache_size = 10000;            // Pages in memory cache
PRAGMA mmap_size = 268435456;         // Memory-mapped I/O (256MB typical)
PRAGMA max_page_count = 4294967295;   // Max pages (limits DB size)
PRAGMA journal_size_limit = -1;       // Max WAL size before checkpoint
PRAGMA temp_store = MEMORY;           // Temp tables in memory vs disk
PRAGMA optimize;                      // Auto-vacuum and index analysis
```

---

## Summary: Design Architecture Diagram

```
CypherLite Single-File Storage Architecture
============================================

┌─────────────────────────────────────────────────────────────┐
│                  .cyl File (Main Database)                  │
├─────────────────────────────────────────────────────────────┤
│ Offset 0-512: File Header                                   │
│  - Magic, version, page size, node/edge counts              │
│  - Root page pointers for all B-trees                       │
│  - Configuration flags and metadata                         │
├─────────────────────────────────────────────────────────────┤
│ Page 1+: Page-Based Storage (4KB pages)                     │
│  ┌──────────────────────────────────────────────────┐       │
│  │ Node Store (B-tree pages)                        │       │
│  │  - Node records (fixed + variable size)          │       │
│  │  - Label bitmaps and inline properties           │       │
│  └──────────────────────────────────────────────────┘       │
│  ┌──────────────────────────────────────────────────┐       │
│  │ Edge Store (B-tree pages)                        │       │
│  │  - Edge records with adjacency pointers          │       │
│  │  - Doubly-linked relationship chains             │       │
│  └──────────────────────────────────────────────────┘       │
│  ┌──────────────────────────────────────────────────┐       │
│  │ Property Store (B-tree pages)                    │       │
│  │  - Property key-value pairs                      │       │
│  │  - Inline small properties, overflow large ones  │       │
│  └──────────────────────────────────────────────────┘       │
│  ┌──────────────────────────────────────────────────┐       │
│  │ Index Pages (B+ trees)                           │       │
│  │  - Label index (label_id → node_id list)         │       │
│  │  - Type index (type_id → edge_id list)           │       │
│  │  - Property indices (key, value → entity list)   │       │
│  └──────────────────────────────────────────────────┘       │
│  ┌──────────────────────────────────────────────────┐       │
│  │ Dictionary Tables (Label, Type, Property Key)    │       │
│  │  - ID to name mappings for quick lookup          │       │
│  └──────────────────────────────────────────────────┘       │
│  ┌──────────────────────────────────────────────────┐       │
│  │ Temporal Pages (B-tree, if enabled)              │       │
│  │  - Anchor + delta versions of nodes/edges        │       │
│  │  - Temporal index for time-range queries         │       │
│  └──────────────────────────────────────────────────┘       │
│  ┌──────────────────────────────────────────────────┐       │
│  │ Free Page List Pages                             │       │
│  │  - Linked list of unallocated pages              │       │
│  └──────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                  .cyl-wal File (WAL Log)                    │
├─────────────────────────────────────────────────────────────┤
│ Header (32 bytes):                                          │
│  - Magic (0x377f0682/0x377f0683)                            │
│  - Page size, checkpoint seq, salt values                   │
├─────────────────────────────────────────────────────────────┤
│ WAL Frames (one per modified page):                         │
│  - Frame header (24 bytes): page num, checksum, commit flag │
│  - Frame payload (4KB): Complete modified page              │
│  - Multiple frames per transaction                          │
│  - Last frame of transaction has commit_marker = 1          │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                .cyl-shm File (Shared Memory Index)          │
├─────────────────────────────────────────────────────────────┤
│ WAL Index (optional, for performance):                      │
│  - Hash table: page_number → frame_index                    │
│  - Helps readers quickly determine page versions in WAL     │
│  - Concurrency locks for multi-process safety              │
└─────────────────────────────────────────────────────────────┘

Data Access Flow:
  Node lookup by ID → B-tree navigation → Slot lookup → Read node record
  Property lookup → Follow first_property_ptr → Walk property chain
  Relationship traversal → Follow next_edge_ptr → O(1) per relationship
  Label query → Label index B-tree → Get node ID list → Fetch nodes
  Type query → Type index B-tree → Get edge ID list → Fetch edges
  Time-based query → Temporal index → Get versions → Apply deltas
```

---

## References and Related Documentation

- **01_existing_technologies.md**: SQLite architecture, Neo4j storage, embedded database analysis
- **02_cypher_rdf_temporal.md**: Cypher language specification, RDF models, temporal semantics
- **Query Engine Design** (future): Cypher parser, planner, execution engine
- **API Design** (future): Public Rust/C/Python APIs for CypherLite
- **Performance Benchmarking** (future): Benchmarking strategy and results

---

**Document Status**: Complete Storage Engine Design
**Next Steps**: Implementation of storage layer, testing, performance optimization

