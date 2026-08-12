# Limitations

- PowerShell 5.1 and PowerShell 7.0–7.2 may reconstruct native command arguments differently from PowerShell 7.3+.
- `.bat` and `.cmd` invoke a second parser and require separate escaping.
- Aliases are not recursively resolved.
- Definitions are intended for interactive shells.
- AllSigned and ConstrainedLanguage environments are not supported by the MVP.
- Alias names are globally unique; different meanings per Shell are not supported.
