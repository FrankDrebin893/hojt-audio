; The HØJT DSP installer for Windows (Inno Setup 6): all six plugins in one setup, built in CI by
; scripts/build-installer.sh from the bundles in target/bundled. Run by hand:
;   iscc /DAppVersion=0.3.0 installer\hojt-dsp.iss
; It installs for the whole machine (admin): VST3 to Common Files\VST3, CLAP to Common Files\CLAP,
; and the licences under Program Files\Hojt Audio. Bundle names and ids are never changed (Versioning
; in AGENTS.md), so a newer setup upgrades in place (the fixed AppId) and projects keep their settings.
;
; Setup takes one parameter of its own, for the tests of the pull-request path (release.yml):
;   /PLUGINROOT=<folder>   puts VST3 and CLAP under <folder>\VST3 and <folder>\CLAP instead of
;                          Common Files, and leaves the machine's plugin folders alone
; Every bundle in bundler.toml has its lines below (xtask/tests/installer.rs holds that).

#ifndef AppVersion
  #error Pass the version: iscc /DAppVersion=<version> (scripts/build-installer.sh reads Cargo.toml)
#endif
#ifndef BundleDir
  #define BundleDir "..\target\bundled"
#endif
#ifndef OutDir
  #define OutDir "..\dist"
#endif
; LICENSE and NOTICE.md with Windows line endings, for the wizard's pages (build-installer.sh)
#ifndef StageDir
  #define StageDir ".."
#endif
#ifndef NumericVersion
  #define NumericVersion AppVersion
#endif

[Setup]
; Never change: it is how a newer setup finds the install it upgrades
AppId={{4DFF5B9E-7443-46F9-9E6B-18D7A4889F6E}
AppName=HØJT DSP
AppVersion={#AppVersion}
AppPublisher=Hojt Audio
AppPublisherURL=https://hojte.net/hojt-audio/dsp/
AppSupportURL=https://github.com/FrankDrebin893/hojt-audio/issues
DefaultDirName={autopf}\Hojt Audio\HOJT DSP
DisableProgramGroupPage=yes
DisableDirPage=auto
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutDir}
OutputBaseFilename=HojtDSP-Setup
LicenseFile={#StageDir}\LICENSE
InfoBeforeFile={#StageDir}\NOTICE.md
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayName=HØJT DSP
VersionInfoVersion={#NumericVersion}
VersionInfoCompany=Hojt Audio
VersionInfoProductName=HØJT DSP
; A running DAW holds the plugins open: Restart Manager asks to close it, and only a file that
; stays locked leaves a restart to offer (restartreplace below)
CloseApplications=yes
CloseApplicationsFilter=*.vst3,*.clap,*.dll,*.exe
RestartApplications=no
#ifdef Sign
; Switched on by build-installer.sh when HOJT_SIGN_COMMAND is set: iscc /Shojtsign="<command> $f"
SignTool=hojtsign
SignedUninstaller=yes
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Types]
Name: "full"; Description: "Every plugin, VST3 and CLAP"
Name: "custom"; Description: "Choose what to install"; Flags: iscustom

[Components]
Name: "pitch_shifter"; Description: "Pitch Shifter"; Types: full
Name: "pitch_shifter\vst3"; Description: "VST3"; Types: full
Name: "pitch_shifter\clap"; Description: "CLAP"; Types: full
Name: "drums"; Description: "Drum Synth"; Types: full
Name: "drums\vst3"; Description: "VST3"; Types: full
Name: "drums\clap"; Description: "CLAP"; Types: full
Name: "amp"; Description: "Guitar Amp"; Types: full
Name: "amp\vst3"; Description: "VST3"; Types: full
Name: "amp\clap"; Description: "CLAP"; Types: full
Name: "bass"; Description: "Bass Synth"; Types: full
Name: "bass\vst3"; Description: "VST3"; Types: full
Name: "bass\clap"; Description: "CLAP"; Types: full
Name: "pad"; Description: "Shimmer Pad"; Types: full
Name: "pad\vst3"; Description: "VST3"; Types: full
Name: "pad\clap"; Description: "CLAP"; Types: full
Name: "bells"; Description: "Glass Bells"; Types: full
Name: "bells\vst3"; Description: "VST3"; Types: full
Name: "bells\clap"; Description: "CLAP"; Types: full

[Files]
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\NOTICE.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\crates\suite_common\assets\fonts\*-OFL.txt"; DestDir: "{app}\licences"; Flags: ignoreversion

; Pitch Shifter
Source: "{#BundleDir}\HojtPitchShifter.vst3\*"; DestDir: "{code:PluginDir|VST3}\HojtPitchShifter.vst3"; Components: pitch_shifter\vst3; Flags: ignoreversion recursesubdirs createallsubdirs restartreplace uninsrestartdelete
Source: "{#BundleDir}\HojtPitchShifter.clap"; DestDir: "{code:PluginDir|CLAP}"; Components: pitch_shifter\clap; Flags: ignoreversion restartreplace uninsrestartdelete

; Drum Synth
Source: "{#BundleDir}\HojtDrumSynth.vst3\*"; DestDir: "{code:PluginDir|VST3}\HojtDrumSynth.vst3"; Components: drums\vst3; Flags: ignoreversion recursesubdirs createallsubdirs restartreplace uninsrestartdelete
Source: "{#BundleDir}\HojtDrumSynth.clap"; DestDir: "{code:PluginDir|CLAP}"; Components: drums\clap; Flags: ignoreversion restartreplace uninsrestartdelete

; Guitar Amp
Source: "{#BundleDir}\HojtGuitarAmp.vst3\*"; DestDir: "{code:PluginDir|VST3}\HojtGuitarAmp.vst3"; Components: amp\vst3; Flags: ignoreversion recursesubdirs createallsubdirs restartreplace uninsrestartdelete
Source: "{#BundleDir}\HojtGuitarAmp.clap"; DestDir: "{code:PluginDir|CLAP}"; Components: amp\clap; Flags: ignoreversion restartreplace uninsrestartdelete

; Bass Synth
Source: "{#BundleDir}\HojtBassSynth.vst3\*"; DestDir: "{code:PluginDir|VST3}\HojtBassSynth.vst3"; Components: bass\vst3; Flags: ignoreversion recursesubdirs createallsubdirs restartreplace uninsrestartdelete
Source: "{#BundleDir}\HojtBassSynth.clap"; DestDir: "{code:PluginDir|CLAP}"; Components: bass\clap; Flags: ignoreversion restartreplace uninsrestartdelete

; Shimmer Pad
Source: "{#BundleDir}\HojtShimmerPad.vst3\*"; DestDir: "{code:PluginDir|VST3}\HojtShimmerPad.vst3"; Components: pad\vst3; Flags: ignoreversion recursesubdirs createallsubdirs restartreplace uninsrestartdelete
Source: "{#BundleDir}\HojtShimmerPad.clap"; DestDir: "{code:PluginDir|CLAP}"; Components: pad\clap; Flags: ignoreversion restartreplace uninsrestartdelete

; Glass Bells
Source: "{#BundleDir}\HojtGlassBells.vst3\*"; DestDir: "{code:PluginDir|VST3}\HojtGlassBells.vst3"; Components: bells\vst3; Flags: ignoreversion recursesubdirs createallsubdirs restartreplace uninsrestartdelete
Source: "{#BundleDir}\HojtGlassBells.clap"; DestDir: "{code:PluginDir|CLAP}"; Components: bells\clap; Flags: ignoreversion restartreplace uninsrestartdelete

[Code]
// Common Files\<Sub>, or <PLUGINROOT>\<Sub> when the tests give one
function PluginDir(Sub: String): String;
begin
  Result := ExpandConstant('{param:PLUGINROOT|}');
  if Result = '' then
    Result := ExpandConstant('{commoncf}');
  Result := AddBackslash(Result) + Sub;
end;
