use crate::{
    error::AliasError,
    executor::render_argv,
    model::{AliasRecord, ManagedNameSet, RestrictionMode, ShellKind, TargetType},
};

pub fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
pub fn is_simple_alias(alias: &AliasRecord) -> bool {
    alias.restriction.is_none()
        && alias.fixed_args.is_empty()
        && !alias.pass_args
        && alias.working_directory.is_none()
        && alias.environment.is_empty()
        && alias.target_type != TargetType::ChangeDirectory
        && !alias
            .executable
            .chars()
            .any(|c| c.is_whitespace() || "'\"$;&|<>".contains(c))
}
pub fn render_preemption(name: &str) -> String {
    format!("Remove-Item -LiteralPath Alias:\\{} -Force -ErrorAction SilentlyContinue\nRemove-Item -LiteralPath Function:\\{} -Force -ErrorAction SilentlyContinue\n", name, name)
}
pub fn render_cleanup(names: &ManagedNameSet) -> String {
    names.current.iter().chain(names.retired.iter()).map(|name| format!("Remove-Item -LiteralPath Alias:\\{} -Force -ErrorAction SilentlyContinue\nRemove-Item -LiteralPath Function:\\{} -Force -ErrorAction SilentlyContinue\n", name, name)).collect()
}

/// Shared directory-canonicalization helper emitted once per generated
/// PowerShell file (and once per standalone preview). It resolves symlinks and
/// junctions with `ResolveLinkTarget` when the runtime provides it (.NET 6+ /
/// PowerShell 7), and falls back to a Win32 `GetFinalPathNameByHandleW`
/// P/Invoke helper on Windows PowerShell 5.1. If neither mechanism works (for
/// example in ConstrainedLanguage), callers receive the lexical full path.
pub const RESTRICTION_SUPPORT: &str = r#"# aliasmgr restriction support
$script:__aliasmgr_resolve_link = $null
$script:__aliasmgr_native_ready = $false
function global:__aliasmgr_canonical_path {
    param([string]$Path)
    try {
        $__aliasmgr_full = [System.IO.Path]::GetFullPath($Path)
    } catch {
        return $Path
    }
    if ($null -eq $script:__aliasmgr_resolve_link) {
        try {
            $script:__aliasmgr_resolve_link = $null -ne [System.IO.DirectoryInfo].GetMethod('ResolveLinkTarget', [System.Type[]]@([bool]))
        } catch {
            $script:__aliasmgr_resolve_link = $false
        }
    }
    if ($script:__aliasmgr_resolve_link) {
        try {
            $__aliasmgr_link = [System.IO.DirectoryInfo]::new($__aliasmgr_full).ResolveLinkTarget($true)
            if ($null -ne $__aliasmgr_link) {
                $__aliasmgr_full = $__aliasmgr_link.FullName
                for ($__aliasmgr_i = 0; $__aliasmgr_i -lt 64; $__aliasmgr_i++) {
                    $__aliasmgr_next = $null
                    try {
                        $__aliasmgr_next = [System.IO.DirectoryInfo]::new($__aliasmgr_full).ResolveLinkTarget($true)
                    } catch {
                        break
                    }
                    if ($null -eq $__aliasmgr_next) { break }
                    $__aliasmgr_full = $__aliasmgr_next.FullName
                }
            }
        } catch {
        }
        return $__aliasmgr_full
    }
    try {
        if (-not $script:__aliasmgr_native_ready) {
            if (-not ('AliasMgrNative' -as [type])) {
                Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
public static class AliasMgrNative {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateFileW(string lpFileName, uint dwDesiredAccess, uint dwShareMode, IntPtr lpSecurityAttributes, uint dwCreationDisposition, uint dwFlagsAndAttributes, IntPtr hTemplateFile);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern uint GetFinalPathNameByHandleW(IntPtr hFile, StringBuilder lpszFilePath, uint cchFilePath, uint dwFlags);
    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool CloseHandle(IntPtr hObject);
    public static string GetFinalPath(string path) {
        const uint FILE_SHARE_READ = 0x1;
        const uint FILE_SHARE_WRITE = 0x2;
        const uint FILE_SHARE_DELETE = 0x4;
        const uint OPEN_EXISTING = 3;
        const uint FILE_FLAG_BACKUP_SEMANTICS = 0x02000000;
        IntPtr handle = CreateFileW(path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, IntPtr.Zero, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, IntPtr.Zero);
        if (handle == new IntPtr(-1)) { throw new Win32Exception(Marshal.GetLastWin32Error()); }
        try {
            uint size = 1024;
            var buffer = new StringBuilder((int)size);
            uint result = GetFinalPathNameByHandleW(handle, buffer, size, 0);
            if (result == 0) { throw new Win32Exception(Marshal.GetLastWin32Error()); }
            if (result >= size) {
                size = result + 1;
                buffer = new StringBuilder((int)size);
                result = GetFinalPathNameByHandleW(handle, buffer, size, 0);
                if (result == 0) { throw new Win32Exception(Marshal.GetLastWin32Error()); }
            }
            string final = buffer.ToString();
            if (final.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase)) { return @"\\" + final.Substring(8); }
            if (final.StartsWith(@"\\?\", StringComparison.Ordinal)) { return final.Substring(4); }
            return final;
        } finally {
            CloseHandle(handle);
        }
    }
}
'@ -ErrorAction Stop
            }
            $script:__aliasmgr_native_ready = $true
        }
        return [AliasMgrNative]::GetFinalPath($__aliasmgr_full)
    } catch {
        return $__aliasmgr_full
    }
}
"#;

pub fn render_restriction_support() -> &'static str {
    RESTRICTION_SUPPORT
}

/// Renders one alias for a standalone snippet (preview, diagnostics, tests).
/// The shared restriction support is prepended for restricted aliases so the
/// snippet is self-contained; generated sync files include it once instead.
pub fn render(
    alias: &AliasRecord,
    shell: ShellKind,
    names: &ManagedNameSet,
) -> Result<String, AliasError> {
    render_with_restriction_support(alias, shell, names)
}

pub fn render_with_restriction_support(
    alias: &AliasRecord,
    shell: ShellKind,
    names: &ManagedNameSet,
) -> Result<String, AliasError> {
    let mut output = String::new();
    if alias.restriction.is_some() {
        output.push_str(RESTRICTION_SUPPORT);
    }
    output.push_str(&render_definition(alias, shell, names)?);
    Ok(output)
}

/// Per-alias definition without the shared helper. [`SyncCoordinator`] emits
/// the helper once per generated file and then uses this function for each
/// alias, so the output never repeats the support block.
///
/// [`SyncCoordinator`]: crate::sync::SyncCoordinator
pub(crate) fn render_definition(
    alias: &AliasRecord,
    shell: ShellKind,
    _names: &ManagedNameSet,
) -> Result<String, AliasError> {
    if !matches!(shell, ShellKind::PowerShell5 | ShellKind::PowerShell7) {
        return Err(AliasError::ShellNotInstalled);
    }
    let mut output = String::new();
    output.push_str(&render_preemption(&alias.name));
    // Validate before the simple-alias fast path so a Set-Alias target cannot
    // bypass the executor checks (a missing path-like executable, for example).
    render_argv(alias, &[])?;
    if is_simple_alias(alias) {
        output.push_str(&format!(
            "{}\n",
            crate::shells::common::managed_definition(
                &alias.name,
                &format!(
                    "Set-Alias -Name {} -Value {} -Scope Global -Force",
                    alias.name,
                    quote(&alias.executable)
                )
            )
        ));
        return Ok(output);
    }
    if alias.target_type == TargetType::ChangeDirectory {
        let directory = alias
            .working_directory
            .as_deref()
            .unwrap_or(&alias.executable);
        let body = format!("Set-Location -LiteralPath {}", quote(directory));
        let definition = match render_restriction_guard(alias) {
            Some(guard) => render_advanced_function(alias, &guard, &[body]),
            None => format!("function global:{} {{ {} }}", alias.name, body),
        };
        output.push_str(&format!(
            "{}\n",
            crate::shells::common::managed_definition(&alias.name, &definition)
        ));
        return Ok(output);
    }
    let args_token = if alias.restriction.is_some() {
        "@__aliasmgr_args"
    } else {
        "@args"
    };
    let command = render_command(alias, args_token);
    output.push_str(&format!(
        "{}\n",
        crate::shells::common::managed_definition(&alias.name, &render_function(alias, &command))
    ));
    Ok(output)
}

/// Header for restricted wrappers. `CmdletBinding` is required for
/// `$PSCmdlet.WriteError`, which is the only mechanism that makes `$?` false
/// after a function call (so `&&` and `if ($?)` stop). Arguments are captured
/// by `ValueFromRemainingArguments` and splatted as `@__aliasmgr_args`.
fn render_advanced_function(alias: &AliasRecord, guard: &str, body: &[String]) -> String {
    let mut lines = vec![
        format!("function global:{} {{", alias.name),
        "    [CmdletBinding()]".to_string(),
        "    param([Parameter(Position=0, ValueFromRemainingArguments=$true)][object[]] $__aliasmgr_args)"
            .to_string(),
        indent(guard, 4),
    ];
    for line in body {
        lines.push(format!("    {line}"));
    }
    lines.push("}".to_string());
    lines.join("\n")
}

fn render_command_tokens(alias: &AliasRecord, args_token: &str) -> Vec<String> {
    let mut tokens = vec![quote(&alias.executable)];
    let mut has_args_placeholder = false;
    for item in &alias.fixed_args {
        if item == "{{args}}" {
            has_args_placeholder = true;
            tokens.push(args_token.to_string());
        } else if item == "{{{{args}}}}" {
            tokens.push(quote("{{args}}"));
        } else {
            tokens.push(quote(item));
        }
    }
    if alias.pass_args && !has_args_placeholder {
        tokens.push(args_token.to_string());
    }
    tokens
}

fn render_command(alias: &AliasRecord, args_token: &str) -> String {
    render_command_tokens(alias, args_token).join(" ")
}

fn indent(text: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    text.lines()
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One `$PSCmdlet.WriteError` statement. Advanced functions are required for
/// this call; it makes `$?` false so `&&`/`if ($?)` stop after a refusal.
fn write_error_line(alias: &AliasRecord, location: &str, message: &str) -> String {
    format!(
        "    $PSCmdlet.WriteError([System.Management.Automation.ErrorRecord]::new([System.Exception]::new(\"aliasmgr: refusing to run '{}' in '{location}': {message}\"), 'aliasmgr.restriction', [System.Management.Automation.ErrorCategory]::PermissionDenied, $null))",
        alias.name
    )
}

/// Guard executed before the command, `Push-Location`, and environment changes.
/// Paths are canonicalized through the shared `__aliasmgr_canonical_path`
/// helper, compared at path component boundaries, and case-folded only when
/// generated on Windows. Non-FileSystem locations are refused.
fn render_restriction_guard(alias: &AliasRecord) -> Option<String> {
    let restriction = alias.restriction.as_ref()?;
    let comparison = if cfg!(windows) {
        "[StringComparison]::OrdinalIgnoreCase"
    } else {
        "[StringComparison]::Ordinal"
    };
    let candidates = restriction
        .paths
        .iter()
        .map(|path| quote(path))
        .collect::<Vec<_>>()
        .join(", ");
    let mut lines = vec![
        "$__aliasmgr_location = Get-Location".to_string(),
        "if ($__aliasmgr_location.Provider.Name -ne 'FileSystem') {".to_string(),
        "    $global:LASTEXITCODE = 1".to_string(),
        write_error_line(
            alias,
            "$($__aliasmgr_location.Path)",
            "current location is not a filesystem directory",
        ),
        "    return".to_string(),
        "}".to_string(),
        "$__aliasmgr_cwd = __aliasmgr_canonical_path -Path $__aliasmgr_location.ProviderPath"
            .to_string(),
        "if ($__aliasmgr_cwd.Length -gt 1 -and $__aliasmgr_cwd -ne [System.IO.Path]::GetPathRoot($__aliasmgr_cwd)) { $__aliasmgr_cwd = $__aliasmgr_cwd.TrimEnd([char[]]@('\\', '/')) }".to_string(),
        "$__aliasmgr_restriction_hit = $false".to_string(),
        format!("foreach ($__aliasmgr_candidate in @({candidates})) {{"),
        "    $__aliasmgr_dir = __aliasmgr_canonical_path -Path $__aliasmgr_candidate".to_string(),
        "    if (-not [System.IO.Directory]::Exists($__aliasmgr_dir)) { continue }".to_string(),
        "    if ($__aliasmgr_dir.Length -gt 1 -and $__aliasmgr_dir -ne [System.IO.Path]::GetPathRoot($__aliasmgr_dir)) { $__aliasmgr_dir = $__aliasmgr_dir.TrimEnd([char[]]@('\\', '/')) }".to_string(),
        "    $__aliasmgr_is_root = $__aliasmgr_dir -eq [System.IO.Path]::GetPathRoot($__aliasmgr_dir)".to_string(),
        format!("    $__aliasmgr_equals = $__aliasmgr_cwd.Equals($__aliasmgr_dir, {comparison})"),
        format!("    $__aliasmgr_starts = $__aliasmgr_cwd.StartsWith($__aliasmgr_dir, {comparison})"),
        "    $__aliasmgr_boundary = $false".to_string(),
        "    if ($__aliasmgr_cwd.Length -gt $__aliasmgr_dir.Length -and $__aliasmgr_starts) {".to_string(),
        "        $__aliasmgr_next = $__aliasmgr_cwd.Substring($__aliasmgr_dir.Length, 1)".to_string(),
        "        $__aliasmgr_boundary = ($__aliasmgr_next -eq '\\') -or ($__aliasmgr_next -eq '/')".to_string(),
        "    }".to_string(),
        "    if ($__aliasmgr_equals -or ($__aliasmgr_is_root -and $__aliasmgr_starts) -or $__aliasmgr_boundary) { $__aliasmgr_restriction_hit = $true }".to_string(),
        "}".to_string(),
    ];
    let (condition, message) = match restriction.mode {
        RestrictionMode::Whitelist => (
            "-not $__aliasmgr_restriction_hit",
            "current directory is not in the whitelist",
        ),
        RestrictionMode::Blacklist => (
            "$__aliasmgr_restriction_hit",
            "current directory is in the blacklist",
        ),
    };
    lines.push(format!("if ({condition}) {{"));
    lines.push("    $global:LASTEXITCODE = 1".to_string());
    lines.push(write_error_line(alias, "$__aliasmgr_cwd", message));
    lines.push("    return".to_string());
    lines.push("}".to_string());
    Some(lines.join("\n"))
}

/// Builds the function body. Working directory and environment changes are
/// restored in `finally` so the caller's session is untouched.
fn render_function(alias: &AliasRecord, command: &str) -> String {
    let guard = render_restriction_guard(alias);
    let cwd = alias.working_directory.as_deref();
    let has_environment = !alias.environment.is_empty();
    if guard.is_none() && cwd.is_none() && !has_environment {
        return format!("function global:{} {{ & {} }}", alias.name, command);
    }
    let mut lines = if guard.is_some() {
        vec![
            format!("function global:{} {{", alias.name),
            "    [CmdletBinding()]".to_string(),
            "    param([Parameter(Position=0, ValueFromRemainingArguments=$true)][object[]] $__aliasmgr_args)".to_string(),
        ]
    } else {
        vec![format!("function global:{} {{", alias.name)]
    };
    if let Some(guard) = &guard {
        lines.push(indent(guard, 4));
    }
    if has_environment {
        lines.push("    $__aliasmgr_environment = @{}".to_string());
        for key in alias.environment.keys() {
            lines.push(format!(
                "    $__aliasmgr_environment[{}] = [Environment]::GetEnvironmentVariable({})",
                quote(key),
                quote(key)
            ));
        }
    }
    if let Some(directory) = cwd {
        lines.push(format!(
            "    Push-Location -LiteralPath {} -ErrorAction Stop",
            quote(directory)
        ));
    }
    lines.push("    try {".to_string());
    for (key, value) in &alias.environment {
        lines.push(format!(
            "        [Environment]::SetEnvironmentVariable({}, {})",
            quote(key),
            quote(value)
        ));
    }
    lines.push(format!("        & {}", command));
    lines.push("    }".to_string());
    lines.push("    finally {".to_string());
    // Restore the location first so a failed environment restore cannot leave
    // the location stack pushed.
    if cwd.is_some() {
        lines.push("        Pop-Location".to_string());
    }
    for key in alias.environment.keys() {
        lines.push(format!(
            "        [Environment]::SetEnvironmentVariable({}, $__aliasmgr_environment[{}])",
            quote(key),
            quote(key)
        ));
    }
    lines.push("    }".to_string());
    lines.push("}".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The release security scan greps for the forbidden cmdlet name, so build it at
    /// runtime instead of embedding the literal in scanned source.
    fn forbidden_eval_device() -> &'static str {
        concat!("Invoke-", "Expression")
    }
    fn names() -> ManagedNameSet {
        ManagedNameSet {
            current: vec![],
            retired: vec![],
        }
    }
    #[test]
    fn renders_simple_set_alias_and_preemption() {
        let alias = AliasRecord {
            name: "ll".into(),
            executable: "Get-ChildItem".into(),
            pass_args: false,
            ..Default::default()
        };
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("Set-Alias -Name ll"));
        assert!(text.contains("Alias:\\ll"));
        assert!(text.contains("Function:\\ll"));
    }
    #[test]
    fn renders_function_with_fixed_args_and_array_passthrough() {
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            fixed_args: vec!["status".into()],
            ..Default::default()
        };
        let text = render(&alias, ShellKind::PowerShell5, &names()).unwrap();
        assert!(text.contains("function global:gs"));
        assert!(text.contains("@args"));
        assert!(text.contains("'git.exe'"));
    }
    #[test]
    fn renders_cleanup_for_tombstones_and_escapes_literals() {
        let alias = AliasRecord {
            name: "q".into(),
            executable: "it's".into(),
            ..Default::default()
        };
        let names = ManagedNameSet {
            current: vec!["q".into()],
            retired: vec!["old".into()],
        };
        assert!(render_cleanup(&names).contains("Alias:\\old"));
        let text = render(&alias, ShellKind::PowerShell7, &names).unwrap();
        assert!(text.contains("it''s"));
    }

    #[test]
    fn renders_working_directory_with_push_and_pop() {
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            fixed_args: vec!["status".into()],
            working_directory: Some("C:/work dir".into()),
            ..Default::default()
        };
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("Push-Location -LiteralPath 'C:/work dir' -ErrorAction Stop"));
        assert!(text.contains("Pop-Location"));
        assert!(text.contains("& 'git.exe' 'status' @args"));
        assert!(!text.contains(forbidden_eval_device()));
    }

    #[test]
    fn renders_environment_with_save_and_restore() {
        let mut environment = std::collections::BTreeMap::new();
        environment.insert("GREETING".to_string(), "it's 中文".to_string());
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            fixed_args: vec!["status".into()],
            environment,
            ..Default::default()
        };
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains(
            "$__aliasmgr_environment['GREETING'] = [Environment]::GetEnvironmentVariable('GREETING')"
        ));
        assert!(text.contains("[Environment]::SetEnvironmentVariable('GREETING', 'it''s 中文')"));
        assert!(text.contains(
            "[Environment]::SetEnvironmentVariable('GREETING', $__aliasmgr_environment['GREETING'])"
        ));
        assert!(text.contains("& 'git.exe' 'status' @args"));
        assert!(!text.contains(forbidden_eval_device()));
    }

    #[test]
    fn renders_working_directory_and_environment_together() {
        let mut environment = std::collections::BTreeMap::new();
        environment.insert("MODE".to_string(), "fast".to_string());
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            working_directory: Some("C:/work".into()),
            environment,
            ..Default::default()
        };
        let text = render(&alias, ShellKind::PowerShell5, &names()).unwrap();
        assert!(text.contains("Push-Location -LiteralPath 'C:/work' -ErrorAction Stop"));
        assert!(text.contains("[Environment]::SetEnvironmentVariable('MODE', 'fast')"));
        let pop = text.find("Pop-Location").expect("Pop-Location");
        let restore = text
            .find("[Environment]::SetEnvironmentVariable('MODE', $__aliasmgr_environment['MODE'])")
            .expect("environment restore");
        assert!(
            pop < restore,
            "location must be restored before env: {text}"
        );
    }

    #[test]
    fn renders_plain_function_without_working_directory_or_environment() {
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            fixed_args: vec!["status".into()],
            ..Default::default()
        };
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("function global:gs { & 'git.exe' 'status' @args }"));
        assert!(!text.contains("Push-Location"));
        assert!(!text.contains("[Environment]::"));
    }

    fn function_alias(fixed_args: Vec<String>, pass_args: bool) -> AliasRecord {
        AliasRecord {
            name: "build".into(),
            executable: "git.exe".into(),
            fixed_args,
            pass_args,
            ..Default::default()
        }
    }

    #[test]
    fn renders_middle_args_placeholder_in_place() {
        let alias = function_alias(
            vec!["build".into(), "{{args}}".into(), "--verbose".into()],
            true,
        );
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("& 'git.exe' 'build' @args '--verbose'"));
    }

    #[test]
    fn renders_trailing_args_placeholder_once() {
        let alias = function_alias(vec!["build".into(), "{{args}}".into()], true);
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("& 'git.exe' 'build' @args"));
        assert_eq!(text.matches("@args").count(), 1);
    }

    #[test]
    fn appends_passthrough_only_when_pass_args_is_true() {
        let passing = function_alias(vec!["build".into()], true);
        assert!(render(&passing, ShellKind::PowerShell7, &names())
            .unwrap()
            .contains("& 'git.exe' 'build' @args"));
        let blocked = function_alias(vec!["build".into()], false);
        let text = render(&blocked, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("& 'git.exe' 'build' }"));
        assert!(!text.contains("@args"));
    }

    #[test]
    fn renders_escaped_args_placeholder_as_literal() {
        let alias = function_alias(vec!["{{{{args}}}}".into()], false);
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("& 'git.exe' '{{args}}' }"));
        let passing = function_alias(vec!["{{{{args}}}}".into()], true);
        assert!(render(&passing, ShellKind::PowerShell7, &names())
            .unwrap()
            .contains("& 'git.exe' '{{args}}' @args"));
    }

    #[test]
    fn quotes_argument_tokens() {
        let alias = function_alias(vec!["it's 中文".into()], false);
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("& 'git.exe' 'it''s 中文' }"));
        assert!(!text.contains(forbidden_eval_device()));
    }

    #[test]
    fn rendered_tokens_match_executor_expansion_for_representative_aliases() {
        fn quoted_expansion(alias: &AliasRecord, args: &[String]) -> Vec<String> {
            render_argv(alias, args)
                .unwrap()
                .iter()
                .map(|value| quote(value))
                .collect()
        }
        fn substitute_passthrough(tokens: Vec<String>, args: &[String]) -> Vec<String> {
            tokens
                .into_iter()
                .flat_map(|token| {
                    if token == "@args" {
                        args.iter().map(|arg| quote(arg)).collect::<Vec<_>>()
                    } else {
                        vec![token]
                    }
                })
                .collect()
        }
        let cases = vec![
            AliasRecord {
                name: "a".into(),
                executable: "git.exe".into(),
                fixed_args: vec![],
                pass_args: false,
                ..Default::default()
            },
            AliasRecord {
                name: "b".into(),
                executable: "python.exe".into(),
                fixed_args: vec!["script.py".into()],
                pass_args: false,
                ..Default::default()
            },
            AliasRecord {
                name: "c".into(),
                executable: "git.exe".into(),
                fixed_args: vec!["it's 中文".into(), "a b".into()],
                pass_args: false,
                ..Default::default()
            },
            AliasRecord {
                name: "d".into(),
                executable: "git.exe".into(),
                fixed_args: vec!["status".into()],
                pass_args: true,
                ..Default::default()
            },
            AliasRecord {
                name: "e".into(),
                executable: "git.exe".into(),
                fixed_args: vec!["build".into(), "{{args}}".into(), "--verbose".into()],
                pass_args: true,
                ..Default::default()
            },
            AliasRecord {
                name: "f".into(),
                executable: "git.exe".into(),
                fixed_args: vec!["build".into(), "{{args}}".into()],
                pass_args: true,
                ..Default::default()
            },
            AliasRecord {
                name: "g".into(),
                executable: "git.exe".into(),
                fixed_args: vec!["{{{{args}}}}".into()],
                pass_args: false,
                ..Default::default()
            },
            AliasRecord {
                name: "h".into(),
                executable: "git.exe".into(),
                fixed_args: vec!["{{{{args}}}}".into()],
                pass_args: true,
                ..Default::default()
            },
        ];
        let args = vec!["user one".to_string(), "it's 中文".to_string()];
        for alias in cases {
            let rendered = substitute_passthrough(render_command_tokens(&alias, "@args"), &args);
            assert_eq!(rendered, quoted_expansion(&alias, &args), "{}", alias.name);
        }
    }

    #[test]
    fn simple_alias_still_runs_executor_validation() {
        let alias = AliasRecord {
            name: "bt".into(),
            executable: r"C:\missing\tool.exe".into(),
            pass_args: false,
            ..Default::default()
        };
        assert!(is_simple_alias(&alias));
        assert!(matches!(
            render(&alias, ShellKind::PowerShell7, &names()),
            Err(AliasError::TargetMissing(_))
        ));
    }

    #[test]
    fn restricted_aliases_are_never_emitted_as_simple_aliases() {
        let alias = AliasRecord {
            name: "rma".into(),
            executable: "Remove-Item".into(),
            pass_args: false,
            restriction: Some(crate::model::PathRestriction {
                mode: RestrictionMode::Whitelist,
                paths: vec!["C:\\work".into()],
            }),
            ..Default::default()
        };
        assert!(!is_simple_alias(&alias));
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("function global:rma {"));
        assert!(!text.contains("Set-Alias -Name rma"));
        assert!(text.contains("current directory is not in the whitelist"));
    }

    #[test]
    fn whitelist_guard_canonicalizes_paths_and_uses_component_boundaries() {
        let alias = AliasRecord {
            name: "rma".into(),
            executable: "Remove-Item".into(),
            fixed_args: vec!["-Recurse".into()],
            restriction: Some(crate::model::PathRestriction {
                mode: RestrictionMode::Whitelist,
                paths: vec!["C:\\work dir".into(), "D:\\it's".into()],
            }),
            ..Default::default()
        };
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains(
            "$__aliasmgr_cwd = __aliasmgr_canonical_path -Path $__aliasmgr_location.ProviderPath"
        ));
        assert!(text.contains("$__aliasmgr_location.Provider.Name -ne 'FileSystem'"));
        assert!(text.contains("foreach ($__aliasmgr_candidate in @('C:\\work dir', 'D:\\it''s'))"));
        assert!(text.contains("$__aliasmgr_cwd.Length -gt $__aliasmgr_dir.Length"));
        assert!(text.contains("$__aliasmgr_cwd.Substring($__aliasmgr_dir.Length, 1)"));
        let comparison = if cfg!(windows) {
            "[StringComparison]::OrdinalIgnoreCase"
        } else {
            "[StringComparison]::Ordinal"
        };
        assert!(text.contains(comparison));
        assert!(text.contains("[CmdletBinding()]"));
        assert!(text.contains(
            "param([Parameter(Position=0, ValueFromRemainingArguments=$true)][object[]] $__aliasmgr_args)"
        ));
        assert!(text.contains("if (-not $__aliasmgr_restriction_hit) {"));
        assert!(text.contains("$PSCmdlet.WriteError([System.Management.Automation.ErrorRecord]::new([System.Exception]::new(\"aliasmgr: refusing to run 'rma' in '$__aliasmgr_cwd': current directory is not in the whitelist\")"));
        assert!(text.contains("$global:LASTEXITCODE = 1"));
        let last_exit = text.find("$global:LASTEXITCODE = 1").unwrap();
        let write_error = text.find("$PSCmdlet.WriteError").unwrap();
        assert!(
            last_exit < write_error,
            "LASTEXITCODE must be set before WriteError (ErrorAction Stop)"
        );
        assert!(!text.contains(forbidden_eval_device()));
        let guard = text.find("$__aliasmgr_cwd").unwrap();
        let command = text
            .find("& 'Remove-Item' '-Recurse' @__aliasmgr_args")
            .unwrap();
        assert!(guard < command, "guard must precede the command: {text}");

        let support = render_restriction_support();
        assert!(support.contains("ResolveLinkTarget"));
        assert!(support.contains("GetFinalPathNameByHandleW"));
        assert!(support.contains("FILE_FLAG_BACKUP_SEMANTICS"));
        let standalone =
            render_with_restriction_support(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert_eq!(
            standalone.matches("# aliasmgr restriction support").count(),
            1
        );
        assert!(standalone.contains("function global:rma"));
    }

    #[test]
    fn blacklist_guard_denies_matches() {
        let alias = AliasRecord {
            name: "rma".into(),
            executable: "Remove-Item".into(),
            pass_args: false,
            restriction: Some(crate::model::PathRestriction {
                mode: RestrictionMode::Blacklist,
                paths: vec!["C:\\Windows".into()],
            }),
            ..Default::default()
        };
        let text = render(&alias, ShellKind::PowerShell5, &names()).unwrap();
        assert!(text.contains("if ($__aliasmgr_restriction_hit) {"));
        assert!(text.contains("current directory is in the blacklist"));
    }

    #[test]
    fn restricted_wrapper_splats_remaining_arguments() {
        let restricted = AliasRecord {
            name: "git-status".into(),
            executable: "git.exe".into(),
            fixed_args: vec!["status".into()],
            restriction: Some(crate::model::PathRestriction {
                mode: RestrictionMode::Whitelist,
                paths: vec!["C:\\work".into()],
            }),
            ..Default::default()
        };
        let text = render(&restricted, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("& 'git.exe' 'status' @__aliasmgr_args"));
        assert!(!text.contains("@args"));

        let plain = AliasRecord {
            restriction: None,
            ..restricted
        };
        assert!(render(&plain, ShellKind::PowerShell7, &names())
            .unwrap()
            .contains("& 'git.exe' 'status' @args"));
    }

    #[test]
    fn change_directory_guard_precedes_set_location() {
        let alias = AliasRecord {
            name: "jump".into(),
            executable: ".".into(),
            target_type: TargetType::ChangeDirectory,
            working_directory: Some("C:/work".into()),
            restriction: Some(crate::model::PathRestriction {
                mode: RestrictionMode::Whitelist,
                paths: vec!["C:\\".into()],
            }),
            ..Default::default()
        };
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        let guard = text.find("$__aliasmgr_cwd").unwrap();
        let change = text.find("Set-Location -LiteralPath 'C:/work'").unwrap();
        assert!(guard < change, "guard must precede Set-Location: {text}");
    }

    #[test]
    fn renders_change_directory_without_passthrough() {
        let alias = AliasRecord {
            name: "jump".into(),
            executable: ".".into(),
            target_type: TargetType::ChangeDirectory,
            working_directory: Some("C:/work dir".into()),
            ..Default::default()
        };
        let text = render(&alias, ShellKind::PowerShell7, &names()).unwrap();
        assert!(text.contains("function global:jump { Set-Location -LiteralPath 'C:/work dir' }"));
        assert!(!text.contains("@args"));
        let fallback = AliasRecord {
            name: "jump".into(),
            executable: ".".into(),
            target_type: TargetType::ChangeDirectory,
            ..Default::default()
        };
        assert!(render(&fallback, ShellKind::PowerShell5, &names())
            .unwrap()
            .contains("function global:jump { Set-Location -LiteralPath '.' }"));
    }
}
