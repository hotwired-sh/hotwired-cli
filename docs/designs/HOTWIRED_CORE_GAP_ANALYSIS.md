# Hotwired-Core Gap Analysis for CLI Implementation

## Executive Summary

This document analyzes what exists in `hotwired-core` versus what the new CLI commands require. The CLI communicates with hotwired-core via Unix socket (`~/.hotwired/hotwired.sock`).

**Key Finding:** Most workflow handlers already exist. The main gaps are:
1. `get_session_state` handler (required by validate.rs)
2. New simplified artifact handlers (replacing 14 old doc_artifact_* handlers)
3. Database schema additions for artifact versioning

---

## 1. Existing Handlers (No Changes Needed)

These handlers already exist and will be used as-is by the CLI:

| Socket Method | Handler | CLI Command |
|---------------|---------|-------------|
| `ping` / `health` | `handlers::health_check` | `hotwired -V` (version check) |
| `list_runs` | `handlers::list_runs` | `hotwired run ls` |
| `get_run_status` | `handlers::get_run_status` | `hotwired run show <id>` |
| `delete_run` | `handlers::delete_run` | `hotwired run rm <id>` |
| `list_active_sessions` | `handlers::list_active_sessions` | `hotwired session ls` |
| `get_conversation_events` | `handlers::get_conversation_events` | `hotwired inbox` |
| `create_event` | `handlers::create_event` | `hotwired send` |
| `report_impediment` | `handlers::report_impediment` | `hotwired impediment` |
| `task_complete` | wrapper for create_event | `hotwired complete` |
| `hotwire` | `handlers::hotwire` | `hotwired hotwire` |
| `pair` | `handlers::pair` | `hotwired pair` |

**Total: 11 handlers working as-is**

---

## 2. New Handlers Required

### 2.1 Session State Handler (CRITICAL)

**Required for:** `validate.rs` - every workflow command calls this first.

```rust
// Socket method: "get_session_state"
// File: handlers/session.rs

pub struct GetSessionStateRequest {
    pub zellij_session: String,  // From $ZELLIJ_SESSION_NAME
}

pub struct GetSessionStateResponse {
    pub attached_run_id: Option<String>,
    pub role_id: Option<String>,
    pub run_status: Option<String>,  // "active", "paused", "completed", etc.
}

pub async fn get_session_state(ctx: &Context, req: GetSessionStateRequest) -> Result<GetSessionStateResponse>
```

**Implementation Logic:**
1. Query `active_claude_sessions` by session_name (from plugin hooks)
2. If not found, query `sessions` table by session_name
3. If session has `attached_run_id`, look up run status
4. Return composite state

**Estimated effort:** 30 minutes

---

### 2.2 Session Show/Remove Handlers

**Required for:** `hotwired session show <name>` and `hotwired session rm <name>`

```rust
// Socket method: "get_session"
// Already exists: handlers::session::get_session()
// Just need to add to route_request()

// Socket method: "deregister_session"
// Already exists: handlers::session::deregister_session()
// Already in route_request() ✓
```

**Only need:** Add `get_session` to socket router (trivial).

---

### 2.3 New Artifact Handlers (MAJOR WORK)

Replace 14 old `doc_artifact_*` handlers with 8 simplified handlers.

#### Current (Being Replaced)
```
doc_artifact_list           → artifact_list
doc_artifact_read           → (REMOVED - use file read)
doc_artifact_create         → (REMOVED - use file create)
doc_artifact_edit           → (REMOVED - use file edit)
doc_artifact_search         → (REMOVED - use grep)
doc_artifact_add_comment    → artifact_add_comment
doc_artifact_resolve_comment → artifact_resolve_comment
doc_artifact_delete_comment  → (REMOVED)
doc_artifact_list_comments  → artifact_list_comments
doc_artifact_suggest_edit   → (REMOVED)
doc_artifact_accept_suggestion → (REMOVED)
doc_artifact_reject_suggestion → (REMOVED)
doc_artifact_list_suggestions → (REMOVED)
doc_artifact_open           → (REMOVED)
```

#### New Handlers Required

```rust
// ========== 1. artifact_list ==========
// Socket method: "artifact_list"
pub struct ArtifactListRequest {
    pub run_id: String,
}

pub struct ArtifactListResponse {
    pub artifacts: Vec<ArtifactInfo>,
}

pub struct ArtifactInfo {
    pub path: String,
    pub status: String,        // "ok" or "missing"
    pub comment_count: i64,
    pub version_count: i64,
    pub title: String,
}

// ========== 2. artifact_sync ==========
// Socket method: "artifact_sync"
pub struct ArtifactSyncRequest {
    pub run_id: String,
    pub path: String,  // Relative to project root
}

pub struct ArtifactSyncResponse {
    pub status: String,           // "registered" or "synced"
    pub title: String,            // Extracted from markdown H1 or filename
    pub version: i64,             // New version number
    pub comments_relocated: i64,  // Comments moved via diffing
    pub comments_orphaned: i64,   // Comments that couldn't be relocated
}

// ========== 3. artifact_move ==========
// Socket method: "artifact_move"
pub struct ArtifactMoveRequest {
    pub run_id: String,
    pub old_path: String,
    pub new_path: String,
    pub refs_only: bool,  // If true, don't move file, just update refs
}

pub struct ArtifactMoveResponse {
    pub file_moved: bool,
    pub comments_preserved: i64,
}

// ========== 4. artifact_add_comment ==========
// Socket method: "artifact_add_comment"
pub struct ArtifactAddCommentRequest {
    pub run_id: String,
    pub path: String,
    pub target_text: String,  // Text to anchor to (not line number!)
    pub comment: String,
    pub author: String,       // Role ID
}

pub struct ArtifactAddCommentResponse {
    pub comment_id: String,
}

// ========== 5. artifact_list_comments ==========
// Socket method: "artifact_list_comments"
pub struct ArtifactListCommentsRequest {
    pub run_id: String,
    pub path: String,
    pub status_filter: String,  // "open", "resolved", "all"
}

pub struct ArtifactListCommentsResponse {
    pub comments: Vec<CommentInfo>,
}

pub struct CommentInfo {
    pub comment_id: String,
    pub target_text: String,
    pub comment: String,
    pub status: String,
    pub author: String,
    pub created_at: String,
}

// ========== 6. artifact_resolve_comment ==========
// Socket method: "artifact_resolve_comment"
pub struct ArtifactResolveCommentRequest {
    pub run_id: String,
    pub comment_id: String,
    pub resolved_by: String,  // Role ID
}

pub struct ArtifactResolveCommentResponse {
    pub success: bool,
}

// ========== 7. artifact_list_versions ==========
// Socket method: "artifact_list_versions"
pub struct ArtifactListVersionsRequest {
    pub run_id: String,
    pub path: String,
}

pub struct ArtifactListVersionsResponse {
    pub versions: Vec<VersionInfo>,
}

pub struct VersionInfo {
    pub version: i64,
    pub timestamp: String,
    pub lines_added: i64,
    pub lines_removed: i64,
}

// ========== 8. artifact_get_version ==========
// Socket method: "artifact_get_version"
pub struct ArtifactGetVersionRequest {
    pub run_id: String,
    pub path: String,
    pub version: i64,
}

pub struct ArtifactGetVersionResponse {
    pub title: String,
    pub content: String,
    pub timestamp: String,
}
```

---

## 3. Database Schema Changes

### 3.1 New Table: `artifact_versions`

```sql
CREATE TABLE artifact_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    artifact_id TEXT NOT NULL,  -- References artifacts.id
    version INTEGER NOT NULL,
    content TEXT NOT NULL,      -- Full document content at this version
    content_hash TEXT NOT NULL, -- SHA-256 for quick comparisons
    lines_added INTEGER DEFAULT 0,
    lines_removed INTEGER DEFAULT 0,
    synced_at TEXT NOT NULL,    -- ISO timestamp

    UNIQUE(artifact_id, version),
    FOREIGN KEY (artifact_id) REFERENCES artifacts(id) ON DELETE CASCADE
);

CREATE INDEX idx_artifact_versions_artifact ON artifact_versions(artifact_id);
```

### 3.2 Modify `artifact_comments` Table

Current schema uses line numbers. Need to change to text-anchored:

```sql
-- Add columns for text anchoring
ALTER TABLE artifact_comments ADD COLUMN target_text TEXT;
ALTER TABLE artifact_comments ADD COLUMN target_hash TEXT;  -- Hash of target_text for matching

-- We can keep line_start/line_end for UI highlighting purposes
-- They get recalculated on each sync based on where target_text appears
```

### 3.3 Existing `artifacts` Table (Minor Changes)

```sql
-- Artifacts table already exists, just ensure these fields:
-- id, run_id, file_path, title, artifact_type, current_version,
-- created_at, updated_at

-- May need to add:
ALTER TABLE artifacts ADD COLUMN current_version INTEGER DEFAULT 1;
```

---

## 4. Implementation Plan

### Phase 1: Session State (BLOCKING - CLI won't work without this)

1. **Add `get_session_state` handler** to `handlers/session.rs`
2. **Add route** in `socket/mod.rs` for `"get_session_state"`
3. **Add types** in `types.rs`

**Files to modify:**
- `packages/hotwired-core/src/handlers/session.rs`
- `packages/hotwired-core/src/socket/mod.rs`
- `packages/hotwired-core/src/types.rs`

### Phase 2: Database Migration

1. **Create migration** for `artifact_versions` table
2. **Add columns** to `artifact_comments` for text anchoring
3. **Add `current_version`** to artifacts if missing

**Files to modify:**
- `packages/hotwired-core/migrations/XXXX_artifact_versions.sql`

### Phase 3: Artifact Handlers

Create new artifact handlers (can coexist with old ones initially):

1. **New file:** `handlers/artifact_v2.rs` (or modify existing)
2. **Add types** for all request/response structs
3. **Add routes** for 8 new methods

**Key implementation details:**

#### artifact_sync
```rust
pub async fn artifact_sync(ctx: &Context, req: ArtifactSyncRequest) -> Result<ArtifactSyncResponse> {
    // 1. Read file from disk (using run's project path)
    // 2. Check if artifact exists in DB
    // 3. If exists:
    //    a. Compute diff against last version
    //    b. Relocate comments using diff
    //    c. Insert new version with content
    // 4. If new:
    //    a. Create artifact record
    //    b. Insert version 1
    // 5. Extract title from H1 or filename
    // 6. Return status
}
```

#### Comment Relocation (Diffing)
```rust
// When artifact_sync finds content changed:
fn relocate_comments(old_content: &str, new_content: &str, comments: Vec<Comment>) -> (Vec<Comment>, i64) {
    // Use similar-text or difflib to find where target_text moved to
    // For each comment:
    //   1. Search new_content for target_text
    //   2. If found: update line numbers, mark relocated
    //   3. If not found: mark orphaned (but keep in DB)
    // Return (relocated_comments, orphaned_count)
}
```

### Phase 4: Socket Router Updates

Add all new methods to `route_request()` in `socket/mod.rs`:

```rust
"get_session_state" => ...
"artifact_list" => ...
"artifact_sync" => ...
"artifact_move" => ...
"artifact_add_comment" => ...
"artifact_list_comments" => ...
"artifact_resolve_comment" => ...
"artifact_list_versions" => ...
"artifact_get_version" => ...
```

### Phase 5: Deprecate Old Handlers

Once new handlers are working:
1. Mark old `doc_artifact_*` methods as deprecated
2. Update MCP to use new handlers (or remove MCP artifact tools entirely)
3. Remove old handlers in next major version

---

## 5. Files to Create/Modify

### New Files
- `packages/hotwired-core/migrations/20240116_artifact_versions.sql`

### Modified Files
- `packages/hotwired-core/src/handlers/session.rs` - add get_session_state
- `packages/hotwired-core/src/handlers/artifact.rs` - add new handlers (or create artifact_v2.rs)
- `packages/hotwired-core/src/handlers/mod.rs` - export new handlers
- `packages/hotwired-core/src/socket/mod.rs` - add routes
- `packages/hotwired-core/src/types.rs` - add request/response types

---

## 6. Testing Strategy

### Unit Tests
- `get_session_state` with various session states
- `artifact_sync` with new file, existing file, changed file
- Comment relocation with moved text, deleted text
- Version history retrieval

### Integration Tests
- Full CLI → socket → handler → DB flow
- Concurrent sync operations
- Error handling (file not found, run not found, etc.)

---

## 7. Estimated Effort

| Component | Effort |
|-----------|--------|
| Phase 1: get_session_state | 1 hour |
| Phase 2: Database migration | 30 mins |
| Phase 3: Artifact handlers | 4-6 hours |
| Phase 4: Socket routes | 30 mins |
| Phase 5: Testing | 2 hours |
| **Total** | **8-10 hours** |

---

## 8. Open Questions

1. **Comment orphaning:** When target_text can't be found, should we:
   - Keep the comment with a warning flag?
   - Move it to "orphaned comments" section?
   - Prompt user for resolution?

2. **Version storage:** Full content per version is simple but can grow large. Options:
   - Store full content (current plan - simple, allows easy retrieval)
   - Store diffs only (smaller, but harder to retrieve specific version)
   - Store full + compress (compromise)

3. **MCP compatibility:** Do we need to keep `doc_artifact_*` handlers working during transition?

---

## Appendix A: Existing Socket Methods Reference

Full list of methods in `socket/mod.rs` `route_request()`:

```
Health:
  - ping / health

Run Management:
  - list_runs
  - get_run_status
  - delete_run
  - request_end_run

Protocol:
  - get_protocol

Conversation:
  - get_conversation_events
  - create_event

Status:
  - report_status
  - update_agent_status

Impediment:
  - report_impediment
  - resolve_impediment

Handoff:
  - handoff

Input:
  - request_input
  - respond_input

MCP Convenience:
  - send_message (wraps create_event)
  - task_complete (wraps create_event)

Doc Artifacts (OLD - to be replaced):
  - doc_artifact_list
  - doc_artifact_read
  - doc_artifact_create
  - doc_artifact_edit
  - doc_artifact_search
  - doc_artifact_add_comment
  - doc_artifact_resolve_comment
  - doc_artifact_list_comments
  - doc_artifact_suggest_edit
  - doc_artifact_accept_suggestion
  - doc_artifact_reject_suggestion
  - doc_artifact_list_suggestions

Session Registration:
  - register_session
  - deregister_session
  - list_active_sessions

Terminal Workflow:
  - list_playbooks
  - list_active_runs
  - hotwire
  - pair
  - request_pair
  - confirm_pending_run
```

## Appendix B: Session Tables Reference

**`sessions` table** - Zellij sessions discovered via detection patterns:
- session_name, tab_name, tab_index, cwd, command
- project_id, workflow_status, attached_run_id, role_id
- agent_type, is_active, is_deleted, last_seen_at

**`active_claude_sessions` table** - Sessions registered by plugin hooks:
- session_name, project_dir, registered_at, last_heartbeat
- git_common_dir, is_worktree

The `get_session_state` handler should query BOTH tables:
1. First check `active_claude_sessions` for the session
2. Cross-reference with `sessions` for run attachment info
