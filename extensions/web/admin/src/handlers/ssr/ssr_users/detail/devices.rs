//! The Devices tab: bridge sessions by presence, certificates and tokens by
//! whether they are still valid.

use chrono::{DateTime, Utc};

use super::context::{DeviceRowView, DevicesTabView};
use super::view::stamp;
use crate::repositories::devices::bridge_presence;
use crate::repositories::users::enrolment::UserDeviceRow;

// Why: a bridge row is a heartbeat, not a credential — it is never revoked,
// so its state is presence (online / idle / stale by the fleet rule); a
// certificate or token is active until revoked. "Active" on the tab counts
// bridges online now plus credentials still valid.
pub(super) fn devices_tab(rows: Vec<UserDeviceRow>) -> DevicesTabView {
    let now = Utc::now();
    let rows: Vec<DeviceRowView> = rows.into_iter().map(|r| device_row(r, now)).collect();
    let active_count = rows.iter().filter(|r| r.is_active).count();
    DevicesTabView {
        count: rows.len(),
        active_count,
        has_rows: !rows.is_empty(),
        rows,
    }
}

fn device_state(row: &UserDeviceRow, now: DateTime<Utc>) -> (&'static str, &'static str, bool) {
    if row.revoked_at.is_some() {
        return ("Revoked", "muted", false);
    }
    if row.kind != "bridge" {
        return ("Active", "ok", true);
    }
    row.last_seen_at.map_or(("Silent", "muted", false), |last| {
        let presence = bridge_presence(now, last);
        (presence.label(), presence.tone(), presence.is_online())
    })
}

fn device_row(row: UserDeviceRow, now: DateTime<Utc>) -> DeviceRowView {
    let revoked = row.revoked_at.is_some();
    let (status_label, status_tone, is_active) = device_state(&row, now);
    DeviceRowView {
        kind_label: match row.kind.as_str() {
            "bridge" => "Bridge",
            "cert" => "Certificate",
            _ => "Token",
        },
        label: row.label,
        detail: row.detail.unwrap_or_default(),
        created_at: stamp(row.created_at),
        last_seen: stamp(row.last_seen_at),
        status_label,
        status_tone,
        is_active,
        revocable: row.revocable && !revoked,
        kind: row.kind,
        id: row.id,
    }
}
