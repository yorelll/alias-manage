import { useRef, useState } from "react";
import { tauriInvoke } from "../lib";

const SHELLS = ["bash", "zsh", "powershell5", "powershell7"] as const;

export function GeneratedFilePanel() {
  const [shell, setShell] = useState<string>("bash");
  const [output, setOutput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const requestToken = useRef(0);

  const changeShell = (value: string) => {
    requestToken.current += 1;
    setShell(value);
    setOutput("");
    setError(null);
  };

  const run = async (command: "generated_preview" | "reload_command") => {
    const token = ++requestToken.current;
    setOutput("");
    setError(null);
    try {
      const result = String(await tauriInvoke(command, { shell }));
      if (requestToken.current === token) setOutput(result);
    } catch (reason) {
      if (requestToken.current === token) setError(String(reason));
    }
  };

  return (
    <section className="panel">
      <h2>Generated file</h2>
      <label>Shell
        <select value={shell} onChange={(event) => changeShell(event.target.value)}>
          {SHELLS.map((value) => <option key={value} value={value}>{value}</option>)}
        </select>
      </label>
      <button onClick={() => run("generated_preview")}>Show generated file</button>
      <button onClick={() => run("reload_command")}>Show reload command</button>
      {error && <p role="alert">{error}</p>}
      {output && <pre>{output}</pre>}
    </section>
  );
}

export default GeneratedFilePanel;
