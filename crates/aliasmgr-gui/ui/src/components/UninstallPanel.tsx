import { useState } from "react";
import { tauriInvoke } from "../lib";

export function UninstallPanel() {
  const [purge, setPurge] = useState(false);
  const [preview, setPreview] = useState<string[]>([]);
  const [armed, setArmed] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectMode = (value: boolean) => {
    setPurge(value);
    setArmed(false);
    setPreview([]);
    setError(null);
  };

  const execute = async () => {
    if (!armed) {
      try {
        setPreview(await tauriInvoke("uninstall_preview", { purge }) as string[]);
        setArmed(true);
        setError(null);
      } catch (reason) {
        setError(String(reason));
      }
      return;
    }
    try {
      setPreview(await tauriInvoke("uninstall_confirm", { purge }) as string[]);
      setArmed(false);
      setError(null);
    } catch (reason) {
      setError(String(reason));
    }
  };

  return <section className="panel"><h1>Uninstall</h1><label><input type="radio" checked={!purge} onChange={() => selectMode(false)} /> Retain aliases</label><label><input type="radio" checked={purge} onChange={() => selectMode(true)} /> Purge aliases</label><p>{purge ? "Generated files, database, and journal will be removed. Referenced target files are protected." : "Generated definitions and target files remain; the loader is removed."}</p>{armed && <p role="alert">{purge ? "Confirm purge to continue." : "Confirm uninstall to continue."}</p>}{preview.map((item) => <p key={item}>{item}</p>)}{error && <p role="alert">{error}</p>}<button onClick={execute}>{armed ? "Confirm uninstall" : purge ? "Review purge" : "Review uninstall"}</button></section>;
}

export default UninstallPanel;
