import { useState } from "react";
import { tauriInvoke } from "../lib";

export function UninstallPanel() {
  const [purge, setPurge] = useState(false);
  const [preview, setPreview] = useState<string[]>([]);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const loadPreview = async (value: boolean) => { setPurge(value); setConfirmed(false); try { setPreview(await tauriInvoke("uninstall_preview") as string[]); } catch (reason) { setError(String(reason)); } };
  const execute = async () => { if (purge && !confirmed) { setConfirmed(true); return; } try { setPreview(await tauriInvoke("uninstall_confirm") as string[]); } catch (reason) { setError(String(reason)); } };
  return <section className="panel"><h1>Uninstall</h1><label><input type="radio" checked={!purge} onChange={() => loadPreview(false)} /> Retain aliases</label><label><input type="radio" checked={purge} onChange={() => loadPreview(true)} /> Purge aliases</label><p>{purge ? "Generated files, database, and journal will be removed. Referenced target files are protected." : "Generated definitions and target files remain; the loader is removed."}</p>{preview.map((item) => <p key={item}>{item}</p>)}{confirmed && <p role="alert">Confirm purge to continue.</p>}{error && <p role="alert">{error}</p>}<button onClick={execute}>{purge && !confirmed ? "Review purge" : "Confirm uninstall"}</button></section>;
}

export default UninstallPanel;
