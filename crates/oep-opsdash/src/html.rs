//! Render a [`DashboardSnapshot`](crate::model::DashboardSnapshot) to a
//! single self-contained HTML page showing all four views: queue depth, node
//! health, job logs and cost/energy.

use std::fmt::Write;

use crate::model::DashboardSnapshot;

/// Escape a string for safe inline embedding in HTML text.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render the snapshot as a full HTML document.
pub fn render_html(snap: &DashboardSnapshot) -> String {
    let queue = &snap.queue;
    let mut h = String::with_capacity(8192);
    let _ = writeln!(
        h,
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>OEP P9 Ops Dashboard</title><style>\
         body{{font-family:system-ui,sans-serif;margin:2rem;color:#1a1a1a;background:#fafafa}}\
         h1{{font-size:1.4rem}} h2{{font-size:1.1rem;margin-top:1.6rem;border-bottom:1px solid #ddd;padding-bottom:.2rem}}\
         table{{border-collapse:collapse;width:100%;font-size:.85rem}}\
         th,td{{text-align:left;padding:.35rem .6rem;border-bottom:1px solid #ececec}}\
         th{{background:#f0f0f0}} .ok{{color:#1a7f37}} .bad{{color:#cf222e}}\
         .warn{{color:#9a6700}} .mono{{font-family:ui-monospace,Menlo,monospace;font-size:.8rem}}\
         .card{{background:#fff;border:1px solid #e2e2e2;border-radius:8px;padding:1rem;margin:.5rem 0}}\
         .stat{{display:inline-block;margin-right:1.4rem}} .stat b{{font-size:1.3rem;display:block}}\
         </style></head><body>"
    );
    let at_ms = snap.at_ms;
    let _ = writeln!(
        h,
        "<h1>OEP P9 Ops Dashboard <span class=\"mono\">(format {} v{})</span></h1>",
        esc(&snap.format),
        snap.version
    );
    let _ = writeln!(h, "<p class=\"mono\">snapshot at t={at_ms}ms</p>");

    // --- queue depth ---
    let _ = writeln!(h, "<h2>Queue depth</h2><div class=\"card\">");
    for (label, val) in [
        ("waiting", queue.waiting),
        ("ready", queue.ready),
        ("running", queue.running),
        ("retrying", queue.retrying),
        ("succeeded", queue.succeeded),
        ("dead-lettered", queue.dead_lettered),
    ] {
        let cls = match label {
            "running" => "ok",
            "dead-lettered" => "bad",
            _ => "",
        };
        let _ = writeln!(
            h,
            "<span class=\"stat\"><b class=\"{cls}\">{val}</b>{label}</span>"
        );
    }
    let _ = writeln!(h, "</div>");

    if !snap.dead_letters.is_empty() {
        let _ = writeln!(h, "<h3>Dead-letter log</h3><table><tr><th>job</th><th>reason</th><th>priority</th><th>attempts</th><th>t</th></tr>");
        for d in &snap.dead_letters {
            let _ = writeln!(
                h,
                "<tr><td class=\"mono\">{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"mono\">{}</td></tr>",
                d.job,
                esc(&d.reason),
                d.priority,
                d.attempts_used,
                d.dead_at_ms
            );
        }
        let _ = writeln!(h, "</table>");
    }

    // --- node health ---
    let _ = writeln!(h, "<h2>Node health</h2><table><tr><th>node</th><th>health</th><th>cpu</th><th>mem</th><th>power W</th><th>running</th><th>succeeded</th><th>reclaimed</th><th>last hb</th></tr>");
    for n in &snap.nodes {
        let cls = match n.health.as_str() {
            "online" => "ok",
            "degraded" => "warn",
            "down" => "bad",
            _ => "",
        };
        let running = n
            .running_jobs
            .iter()
            .map(|j| j.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            h,
            "<tr><td class=\"mono\">{}</td><td class=\"{cls}\">{}</td><td>{}</td><td>{}</td><td>{}</td>\
             <td class=\"mono\">{}</td><td>{}</td><td>{}</td><td class=\"mono\">{}</td></tr>",
            esc(&n.id),
            esc(&n.health),
            n.cpu_millicores,
            n.memory_mib,
            n.power_w,
            running,
            n.jobs_succeeded,
            n.jobs_reclaimed,
            n.last_heartbeat_ms
        );
    }
    let _ = writeln!(h, "</table>");

    // --- job logs ---
    let _ = writeln!(h, "<h2>Job logs</h2><table><tr><th>t</th><th>job</th><th>node</th><th>event</th><th>detail</th></tr>");
    for e in snap.log.iter().rev().take(200) {
        let job = e.job.map(|j| j.to_string()).unwrap_or_default();
        let node = e.node.clone().unwrap_or_default();
        let cls = match e.kind.as_str() {
            "ack" => "ok",
            "dead-letter" | "poison" | "reclaim" => "bad",
            "fail" => "warn",
            _ => "",
        };
        let _ = writeln!(
            h,
            "<tr><td class=\"mono\">{}</td><td class=\"mono\">{}</td><td class=\"mono\">{}</td>\
             <td class=\"{cls}\">{}</td><td>{}</td></tr>",
            e.at_ms,
            esc(&job),
            esc(&node),
            esc(&e.kind),
            esc(&e.detail)
        );
    }
    let _ = writeln!(h, "</table>");

    // --- cost/energy ---
    let _ = writeln!(h, "<h2>Cost / Energy</h2><div class=\"card\">");
    let _ = writeln!(
        h,
        "<span class=\"stat\"><b>{}</b>energy mWh</span><span class=\"stat\"><b>{}</b>cost micro-USD</span><span class=\"stat\"><b>{}</b>runs</span>",
        snap.cost_totals.energy_mwh,
        snap.cost_totals.cost_usd_micro,
        snap.cost_totals.runs
    );
    let _ = writeln!(h, "</div><table><tr><th>node</th><th>energy mWh</th><th>cost micro-USD</th><th>runs</th></tr>");
    for c in snap.cost.values() {
        let _ = writeln!(
            h,
            "<tr><td class=\"mono\">{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&c.node),
            c.energy_mwh,
            c.cost_usd_micro,
            c.runs
        );
    }
    let _ = writeln!(h, "</table></body></html>");
    h
}
