import { useEffect, useState } from "react";
import { tauriInvoke } from "../lib";

export function DoctorPanel() {
  const [findings, setFindings] = useState<Array<{ shell?: string; severity: string; message: string; action?: string }>>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { tauriInvoke("doctor_status").then((value) => setFindings(value as typeof findings)).catch((reason) => setError(String(reason))); }, []);
  const counts = findings.reduce<Record<string, number>>((result, finding) => ({ ...result, [finding.severity]: (result[finding.severity] ?? 0) + 1 }), {});
  return <section className="panel"><h1>Doctor</h1>{error && <p role="alert">Unable to load diagnostics: {error}</p>}<div className="severity-summary">{Object.entries(counts).map(([severity, count]) => <span key={severity}>{severity}: {count}</span>)}</div>{findings.map((finding, index) => <details key={`${finding.shell ?? "global"}-${index}`}><summary>{finding.shell ?? "Global"}: {finding.severity}</summary><p>{finding.message}</p></details>)}{!findings.length && !error && <p>No diagnostic findings.</p>}</section>;
}

export default DoctorPanel;
