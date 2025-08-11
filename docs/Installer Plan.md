# Seen Language Cross-Platform Installer

## Installation Architecture

### Distribution Channels

1. **Native Installers** (recommended for most users)
    - Windows: MSI installer with digital signature
    - Linux: Native packages (deb, rpm, AppImage)
    - macOS: DMG with notarization

2. **Universal Script** (for CI/CD and power users)
    - Single-line curl/PowerShell installation
    - Auto-detects platform and architecture
    - Supports custom installation paths

3. **Package Managers** (for ecosystem integration)
    - Cargo (during bootstrap phase)
    - Homebrew (macOS/Linux)
    - Scoop/Chocolatey (Windows)
    - APT/YUM/DNF repositories

## Windows Installer Implementation

### MSI Installer with WiX Toolset

```xml
<!-- installer/windows/seen.wxs -->
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" 
           Name="Seen Programming Language" 
           Language="1033" 
           Version="1.0.0.0" 
           Manufacturer="Seen Language Team"
           UpgradeCode="A7B3C5D7-9F2E-4A6B-8C1D-E3F5A7B9C1D3">
    
    <Package InstallerVersion="500" 
             Compressed="yes" 
             InstallScope="perMachine"
             Platform="x64" />
    
    <MediaTemplate EmbedCab="yes" />
    
    <!-- Detect previous versions -->
    <MajorUpgrade DowngradeErrorMessage="A newer version is already installed." />
    
    <!-- Windows version check -->
    <Condition Message="Windows 10 or higher is required.">
      <![CDATA[Installed OR (VersionNT64 >= 603)]]>
    </Condition>
    
    <!-- Installation directory -->
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLFOLDER" Name="Seen">
          
          <!-- Core compiler binaries -->
          <Component Id="CompilerBinaries" Guid="B8C4D6E8-A0F3-5B7C-9D2E-F4A6B8C1D2E4">
            <File Id="SeenExe" Source="$(var.BuildDir)\seen.exe" KeyPath="yes">
              <Shortcut Id="SeenShortcut" 
                       Directory="ProgramMenuFolder" 
                       Name="Seen Compiler"
                       WorkingDirectory="INSTALLFOLDER"
                       Icon="seen.ico"
                       Advertise="yes" />
            </File>
            <File Id="SeenLspExe" Source="$(var.BuildDir)\seen-lsp.exe" />
            
            <!-- RISC-V cross-compilation tools -->
            <File Id="RiscVToolchain" Source="$(var.BuildDir)\riscv-tools.exe" />
            
            <!-- Environment variable -->
            <Environment Id="PATH" 
                        Name="PATH" 
                        Value="[INSTALLFOLDER]" 
                        Permanent="no" 
                        Part="last" 
                        Action="set" 
                        System="yes" />
            
            <!-- Registry entries for file associations -->
            <RegistryValue Root="HKLM" 
                          Key="Software\SeenLang\Compiler" 
                          Name="InstallPath" 
                          Type="string" 
                          Value="[INSTALLFOLDER]" />
            
            <ProgId Id="SeenFile" Description="Seen Source File" Icon="seen.ico">
              <Extension Id="seen" ContentType="text/x-seen">
                <Verb Id="open" Command="Open with Seen" 
                     TargetFile="SeenExe" 
                     Argument='"%1"' />
              </Extension>
            </ProgId>
          </Component>
          
          <!-- Standard library -->
          <Directory Id="StdLibDir" Name="stdlib">
            <Component Id="StandardLibrary" Guid="C9D5E7F9-B1F4-6C8D-A3E5-F5B7C9D1E3F5">
              <File Id="StdCore" Source="$(var.StdLibDir)\core.seen" KeyPath="yes" />
              <File Id="StdReactive" Source="$(var.StdLibDir)\reactive.seen" />
              <File Id="StdCollections" Source="$(var.StdLibDir)\collections.seen" />
              <!-- Additional stdlib files -->
            </Component>
          </Directory>
          
          <!-- Language configuration files -->
          <Directory Id="LangDir" Name="languages">
            <Component Id="LanguageConfigs" Guid="D0E6F8A0-C2F5-7D9E-B4F6-F6C8D0E2F6A6">
              <File Id="EnToml" Source="$(var.LangDir)\en.toml" KeyPath="yes" />
              <File Id="ArToml" Source="$(var.LangDir)\ar.toml" />
            </Component>
          </Directory>
          
          <!-- Documentation -->
          <Directory Id="DocDir" Name="docs">
            <Component Id="Documentation" Guid="E1F7A9B1-D3F6-8E0F-C5F7-F7D9E1F3F7B7">
              <File Id="GettingStarted" Source="$(var.DocDir)\getting-started.md" KeyPath="yes" />
              <File Id="LanguageRef" Source="$(var.DocDir)\language-reference.pdf" />
            </Component>
          </Directory>
        </Directory>
      </Directory>
      
      <!-- Start Menu -->
      <Directory Id="ProgramMenuFolder">
        <Directory Id="ApplicationProgramsFolder" Name="Seen Language" />
      </Directory>
    </Directory>
    
    <!-- Features -->
    <Feature Id="Complete" Title="Seen Language" Level="1" Display="expand">
      <Feature Id="Compiler" Title="Compiler and Tools" Level="1" Absent="disallow">
        <ComponentRef Id="CompilerBinaries" />
      </Feature>
      <Feature Id="StdLib" Title="Standard Library" Level="1">
        <ComponentRef Id="StandardLibrary" />
      </Feature>
      <Feature Id="Languages" Title="Language Configurations" Level="1">
        <ComponentRef Id="LanguageConfigs" />
      </Feature>
      <Feature Id="Docs" Title="Documentation" Level="1">
        <ComponentRef Id="Documentation" />
      </Feature>
    </Feature>
    
    <!-- Custom Actions -->
    <CustomAction Id="VerifyVSRedist" 
                  BinaryKey="CustomActionDLL" 
                  DllEntry="CheckVCRedist" 
                  Execute="immediate" />
    
    <InstallExecuteSequence>
      <Custom Action="VerifyVSRedist" Before="InstallFiles">NOT Installed</Custom>
    </InstallExecuteSequence>
    
    <!-- UI -->
    <UIRef Id="WixUI_FeatureTree" />
    <UIRef Id="WixUI_ErrorProgressText" />
    
    <!-- License -->
    <WixVariable Id="WixUILicenseRtf" Value="$(var.ProjectDir)\LICENSE.rtf" />
    <WixVariable Id="WixUIBannerBmp" Value="$(var.ProjectDir)\installer\banner.bmp" />
    <WixVariable Id="WixUIDialogBmp" Value="$(var.ProjectDir)\installer\dialog.bmp" />
    
    <!-- Icon -->
    <Icon Id="seen.ico" SourceFile="$(var.ProjectDir)\assets\seen.ico" />
  </Product>
</Wix>
```

### PowerShell Installation Script

```powershell
# installer/windows/install.ps1
param(
    [string]$Version = "latest",
    [string]$InstallDir = "$env:ProgramFiles\Seen",
    [switch]$AddToPath = $true,
    [switch]$NoStdLib = $false,
    [string]$Architecture = "x64"  # x64, arm64, riscv64
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

function Write-ColorOutput($ForegroundColor) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    if ($args) {
        Write-Output $args
    }
    $host.UI.RawUI.ForegroundColor = $fc
}

Write-ColorOutput Green "==============================================="
Write-ColorOutput Green "     Seen Language Installer for Windows      "
Write-ColorOutput Green "==============================================="

# Check Windows version
$osVersion = [System.Environment]::OSVersion.Version
if ($osVersion.Major -lt 10) {
    Write-ColorOutput Red "Error: Windows 10 or higher is required"
    exit 1
}

# Check for admin privileges
$currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
$isAdmin = $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-ColorOutput Yellow "Warning: Not running as administrator. PATH update may fail."
    $response = Read-Host "Continue anyway? (y/n)"
    if ($response -ne 'y') { exit 0 }
}

# Determine download URL
$baseUrl = "https://github.com/seen-lang/seen/releases"
if ($Version -eq "latest") {
    $releaseUrl = "$baseUrl/latest/download"
} else {
    $releaseUrl = "$baseUrl/download/v$Version"
}

$fileName = "seen-$Version-windows-$Architecture.zip"
$downloadUrl = "$releaseUrl/$fileName"

Write-Host "Downloading Seen $Version for Windows ($Architecture)..."

# Create temp directory
$tempDir = Join-Path $env:TEMP "seen-installer-$(Get-Random)"
New-Item -ItemType Directory -Path $tempDir | Out-Null

try {
    # Download the release
    $zipPath = Join-Path $tempDir $fileName
    Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath
    
    Write-Host "Extracting files..."
    
    # Extract archive
    Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force
    
    # Stop any running Seen LSP servers
    Get-Process -Name "seen-lsp" -ErrorAction SilentlyContinue | Stop-Process -Force
    
    # Create installation directory
    if (Test-Path $InstallDir) {
        Write-Host "Removing previous installation..."
        Remove-Item -Path $InstallDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    
    # Copy files
    Write-Host "Installing Seen to $InstallDir..."
    Copy-Item -Path "$tempDir\*" -Destination $InstallDir -Recurse -Force
    
    # Set up file associations
    Write-Host "Setting up file associations..."
    $seenExe = Join-Path $InstallDir "seen.exe"
    
    # Register .seen file extension
    New-Item -Path "HKCU:\Software\Classes\.seen" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\.seen" -Name "(Default)" -Value "SeenFile"
    
    New-Item -Path "HKCU:\Software\Classes\SeenFile" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\SeenFile" -Name "(Default)" -Value "Seen Source File"
    
    New-Item -Path "HKCU:\Software\Classes\SeenFile\DefaultIcon" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\SeenFile\DefaultIcon" -Name "(Default)" -Value "$seenExe,0"
    
    New-Item -Path "HKCU:\Software\Classes\SeenFile\shell\open\command" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\SeenFile\shell\open\command" -Name "(Default)" -Value "`"$seenExe`" `"%1`""
    
    # Add to PATH
    if ($AddToPath) {
        Write-Host "Adding Seen to PATH..."
        $currentPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
        if ($currentPath -notlike "*$InstallDir*") {
            $newPath = "$currentPath;$InstallDir"
            [Environment]::SetEnvironmentVariable("Path", $newPath, [EnvironmentVariableTarget]::User)
            $env:Path = $newPath
        }
    }
    
    # Install Visual C++ Redistributables if needed
    Write-Host "Checking for Visual C++ Redistributables..."
    $vcRedistKey = "HKLM:\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\$Architecture"
    if (-not (Test-Path $vcRedistKey)) {
        Write-ColorOutput Yellow "Visual C++ Redistributables not found. Installing..."
        $vcRedistUrl = "https://aka.ms/vs/17/release/vc_redist.$Architecture.exe"
        $vcRedistPath = Join-Path $tempDir "vc_redist.exe"
        Invoke-WebRequest -Uri $vcRedistUrl -OutFile $vcRedistPath
        Start-Process -FilePath $vcRedistPath -ArgumentList "/quiet", "/norestart" -Wait
    }
    
    # Verify installation
    Write-Host "Verifying installation..."
    $version = & $seenExe --version
    
    Write-ColorOutput Green "==============================================="
    Write-ColorOutput Green "     Installation completed successfully!      "
    Write-ColorOutput Green "==============================================="
    Write-Host ""
    Write-Host "Seen $version has been installed to: $InstallDir"
    Write-Host ""
    Write-Host "To get started, run:"
    Write-ColorOutput Cyan "  seen init my-project"
    Write-ColorOutput Cyan "  cd my-project"
    Write-ColorOutput Cyan "  seen build"
    Write-Host ""
    Write-Host "For VS Code support, install the extension:"
    Write-ColorOutput Cyan "  code --install-extension seen-lang.seen-vscode"
    
} catch {
    Write-ColorOutput Red "Installation failed: $_"
    exit 1
} finally {
    # Cleanup
    if (Test-Path $tempDir) {
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
```

## Linux Installer Implementation

### Universal Shell Script

```bash
#!/usr/bin/env bash
# installer/linux/install.sh

set -e

# Configuration
VERSION="${VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local}"
ARCH="${ARCH:-$(uname -m)}"
ADD_TO_PATH="${ADD_TO_PATH:-true}"
INSTALL_STDLIB="${INSTALL_STDLIB:-true}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Functions
print_header() {
    echo -e "${GREEN}===============================================${NC}"
    echo -e "${GREEN}     Seen Language Installer for Linux        ${NC}"
    echo -e "${GREEN}===============================================${NC}"
}

error() {
    echo -e "${RED}Error: $1${NC}" >&2
    exit 1
}

warning() {
    echo -e "${YELLOW}Warning: $1${NC}"
}

info() {
    echo -e "${BLUE}$1${NC}"
}

success() {
    echo -e "${GREEN}$1${NC}"
}

detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        echo "$ID"
    elif type lsb_release >/dev/null 2>&1; then
        lsb_release -si | tr '[:upper:]' '[:lower:]'
    else
        echo "unknown"
    fi
}

detect_architecture() {
    case "$ARCH" in
        x86_64|amd64)
            echo "x86_64"
            ;;
        aarch64|arm64)
            echo "aarch64"
            ;;
        riscv64)
            echo "riscv64"
            ;;
        *)
            error "Unsupported architecture: $ARCH"
            ;;
    esac
}

check_dependencies() {
    local deps=("curl" "tar")
    local missing=()
    
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            missing+=("$dep")
        fi
    done
    
    if [ ${#missing[@]} -gt 0 ]; then
        error "Missing dependencies: ${missing[*]}. Please install them first."
    fi
}

download_release() {
    local version="$1"
    local arch="$2"
    local temp_dir="$3"
    
    local base_url="https://github.com/seen-lang/seen/releases"
    if [ "$version" = "latest" ]; then
        local url="$base_url/latest/download/seen-latest-linux-${arch}.tar.gz"
    else
        local url="$base_url/download/v${version}/seen-${version}-linux-${arch}.tar.gz"
    fi
    
    info "Downloading Seen $version for Linux ($arch)..."
    curl -L -o "$temp_dir/seen.tar.gz" "$url" || error "Failed to download Seen"
}

install_binary_package() {
    local temp_dir="$1"
    local install_dir="$2"
    
    info "Extracting files..."
    tar -xzf "$temp_dir/seen.tar.gz" -C "$temp_dir"
    
    info "Installing Seen to $install_dir..."
    
    # Create directories
    sudo mkdir -p "$install_dir/bin"
    sudo mkdir -p "$install_dir/lib/seen"
    sudo mkdir -p "$install_dir/share/seen"
    
    # Install binaries
    sudo cp "$temp_dir/seen" "$install_dir/bin/"
    sudo cp "$temp_dir/seen-lsp" "$install_dir/bin/"
    sudo chmod +x "$install_dir/bin/seen"
    sudo chmod +x "$install_dir/bin/seen-lsp"
    
    # Install RISC-V cross-compilation tools if present
    if [ -f "$temp_dir/seen-riscv" ]; then
        sudo cp "$temp_dir/seen-riscv" "$install_dir/bin/"
        sudo chmod +x "$install_dir/bin/seen-riscv"
    fi
    
    # Install standard library
    if [ "$INSTALL_STDLIB" = "true" ]  and  [ -d "$temp_dir/stdlib" ]; then
        sudo cp -r "$temp_dir/stdlib" "$install_dir/lib/seen/"
    fi
    
    # Install language configurations
    if [ -d "$temp_dir/languages" ]; then
        sudo cp -r "$temp_dir/languages" "$install_dir/share/seen/"
    fi
    
    # Install documentation
    if [ -d "$temp_dir/docs" ]; then
        sudo cp -r "$temp_dir/docs" "$install_dir/share/seen/"
    fi
    
    # Install man pages
    if [ -f "$temp_dir/seen.1" ]; then
        sudo mkdir -p "$install_dir/share/man/man1"
        sudo cp "$temp_dir/seen.1" "$install_dir/share/man/man1/"
        sudo gzip -f "$install_dir/share/man/man1/seen.1"
    fi
}

create_native_package() {
    local distro="$1"
    local temp_dir="$2"
    
    case "$distro" in
        ubuntu|debian)
            create_deb_package "$temp_dir"
            ;;
        fedora|rhel|centos)
            create_rpm_package "$temp_dir"
            ;;
        arch|manjaro)
            create_pacman_package "$temp_dir"
            ;;
        *)
            warning "Native package not available for $distro, using binary installation"
            return 1
            ;;
    esac
}

create_deb_package() {
    local temp_dir="$1"
    
    info "Creating Debian package..."
    
    mkdir -p "$temp_dir/debian/DEBIAN"
    mkdir -p "$temp_dir/debian/usr/bin"
    mkdir -p "$temp_dir/debian/usr/lib/seen"
    mkdir -p "$temp_dir/debian/usr/share/seen"
    
    # Copy files
    cp "$temp_dir/seen" "$temp_dir/debian/usr/bin/"
    cp "$temp_dir/seen-lsp" "$temp_dir/debian/usr/bin/"
    cp -r "$temp_dir/stdlib" "$temp_dir/debian/usr/lib/seen/" 2>/dev/null || true
    cp -r "$temp_dir/languages" "$temp_dir/debian/usr/share/seen/" 2>/dev/null || true
    
    # Create control file
    cat > "$temp_dir/debian/DEBIAN/control" << EOF
Package: seen-lang
Version: $VERSION
Architecture: $(dpkg --print-architecture)
Maintainer: Seen Language Team <team@seen-lang.org>
Description: The Seen Programming Language
 A high-performance, multi-paradigm programming language
 with reactive programming support and RISC-V optimization.
Depends: libc6 (>= 2.27)
Priority: optional
Section: devel
EOF
    
    # Create postinst script
    cat > "$temp_dir/debian/DEBIAN/postinst" << 'EOF'
#!/bin/bash
set -e
ldconfig
update-alternatives --install /usr/bin/seen seen /usr/bin/seen 100
EOF
    chmod 755 "$temp_dir/debian/DEBIAN/postinst"
    
    # Build package
    dpkg-deb --build "$temp_dir/debian" "$temp_dir/seen-lang.deb"
    
    # Install package
    info "Installing Debian package..."
    sudo dpkg -i "$temp_dir/seen-lang.deb"
}

create_rpm_package() {
    local temp_dir="$1"
    
    info "Creating RPM package..."
    
    # Create RPM build structure
    mkdir -p "$temp_dir/rpmbuild"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
    
    # Create tarball for sources
    tar -czf "$temp_dir/rpmbuild/SOURCES/seen-$VERSION.tar.gz" -C "$temp_dir" .
    
    # Create spec file
    cat > "$temp_dir/rpmbuild/SPECS/seen.spec" << EOF
Name:           seen-lang
Version:        $VERSION
Release:        1%{?dist}
Summary:        The Seen Programming Language
License:        MIT
URL:            https://seen-lang.org
Source0:        seen-%{version}.tar.gz

%description
A high-performance, multi-paradigm programming language
with reactive programming support and RISC-V optimization.

%prep
%setup -q

%install
mkdir -p %{buildroot}%{_bindir}
mkdir -p %{buildroot}%{_libdir}/seen
mkdir -p %{buildroot}%{_datadir}/seen

cp seen %{buildroot}%{_bindir}/
cp seen-lsp %{buildroot}%{_bindir}/
cp -r stdlib %{buildroot}%{_libdir}/seen/
cp -r languages %{buildroot}%{_datadir}/seen/

%files
%{_bindir}/seen
%{_bindir}/seen-lsp
%{_libdir}/seen/
%{_datadir}/seen/

%post
/sbin/ldconfig

%postun
/sbin/ldconfig
EOF
    
    # Build RPM
    rpmbuild --define "_topdir $temp_dir/rpmbuild" -bb "$temp_dir/rpmbuild/SPECS/seen.spec"
    
    # Install RPM
    info "Installing RPM package..."
    sudo rpm -i "$temp_dir/rpmbuild/RPMS/"*/*.rpm
}

setup_path() {
    local install_dir="$1"
    
    if [ "$ADD_TO_PATH" != "true" ]; then
        return
    fi
    
    info "Setting up PATH..."
    
    local shell_rc=""
    if [ -n "$BASH_VERSION" ]; then
        shell_rc="$HOME/.bashrc"
    elif [ -n "$ZSH_VERSION" ]; then
        shell_rc="$HOME/.zshrc"
    elif [ -f "$HOME/.profile" ]; then
        shell_rc="$HOME/.profile"
    fi
    
    if [ -n "$shell_rc" ]; then
        if ! grep -q "$install_dir/bin" "$shell_rc" 2>/dev/null; then
            echo "export PATH=\"$install_dir/bin:\$PATH\"" >> "$shell_rc"
            info "Added $install_dir/bin to PATH in $shell_rc"
        fi
    fi
    
    # Update current session
    export PATH="$install_dir/bin:$PATH"
}

verify_installation() {
    if command -v seen &> /dev/null; then
        local version=$(seen --version 2>&1)
        success "Seen $version installed successfully!"
        return 0
    else
        error "Installation verification failed"
    fi
}

cleanup() {
    if [ -n "$1" ]  and  [ -d "$1" ]; then
        rm -rf "$1"
    fi
}

main() {
    print_header
    
    # Check for root/sudo when needed
    if [ "$INSTALL_DIR" = "/usr/local" ] || [ "$INSTALL_DIR" = "/usr" ]; then
        if [ "$EUID" -ne 0 ]  and  ! sudo -n true 2>/dev/null; then
            error "This script requires sudo privileges for system-wide installation"
        fi
    fi
    
    check_dependencies
    
    local distro=$(detect_distro)
    local arch=$(detect_architecture)
    local temp_dir=$(mktemp -d)
    
    trap "cleanup $temp_dir" EXIT
    
    info "Detected: $distro Linux on $arch"
    
    # Download release
    download_release "$VERSION" "$arch" "$temp_dir"
    
    # Try native package first, fall back to binary
    if ! create_native_package "$distro" "$temp_dir"; then
        install_binary_package "$temp_dir" "$INSTALL_DIR"
    fi
    
    setup_path "$INSTALL_DIR"
    verify_installation
    
    echo ""
    success "==============================================="
    success "     Installation completed successfully!      "
    success "==============================================="
    echo ""
    echo "To get started, run:"
    echo -e "${BLUE}  seen init my-project${NC}"
    echo -e "${BLUE}  cd my-project${NC}"
    echo -e "${BLUE}  seen build${NC}"
    echo ""
    echo "For VS Code support, install the extension:"
    echo -e "${BLUE}  code --install-extension seen-lang.seen-vscode${NC}"
}

# Run main function
main "$@"
```

### AppImage for Universal Linux

```yaml
# installer/linux/appimage/seen.yml
app: seen
version: 1.0.0

AppDir:
  path: ./AppDir
  
  app_info:
    id: org.seen-lang.seen
    name: Seen
    icon: seen
    version: 1.0.0
    exec: bin/seen
    exec_args: $@
  
  runtime:
    version: continuous
    arch: x86_64
    
  apt:
    arch: amd64
    sources:
      - sourceline: 'deb [arch=amd64] http://archive.ubuntu.com/ubuntu/ focal main restricted universe multiverse'
    include:
      - libc6
      - libgcc-s1
      - libstdc++6
    
  files:
    include:
      - bin/seen
      - bin/seen-lsp
      - lib/seen/
      - share/seen/
    exclude:
      - usr/share/man
      - usr/share/doc
  
  test:
    fedora-30:
      image: fedora:30
      command: "./AppRun --version"
    debian-stable:
      image: debian:stable
      command: "./AppRun --version"
    arch-latest:
      image: archlinux:latest
      command: "./AppRun --version"
    centos-7:
      image: centos:7
      command: "./AppRun --version"
    ubuntu-18.04:
      image: ubuntu:18.04
      command: "./AppRun --version"

AppImage:
  arch: x86_64
  update-information: gh-releases-zsync|seen-lang|seen|latest|Seen-*x86_64.AppImage.zsync
```

## Package Manager Integration

### Homebrew Formula (macOS/Linux)

```ruby
# installer/homebrew/seen.rb
class Seen < Formula
  desc "High-performance multi-paradigm programming language"
  homepage "https://seen-lang.org"
  version "1.0.0"
  
  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-darwin-arm64.tar.gz"
      sha256 "YOUR_SHA256_HERE"
    else
      url "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-darwin-x64.tar.gz"
      sha256 "YOUR_SHA256_HERE"
    end
  elsif OS.linux?
    if Hardware::CPU.arm?
      url "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-linux-arm64.tar.gz"
      sha256 "YOUR_SHA256_HERE"
    else
      url "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-linux-x64.tar.gz"
      sha256 "YOUR_SHA256_HERE"
    end
  end
  
  depends_on :macos => :catalina if OS.mac?
  
  def install
    bin.install "seen", "seen-lsp"
    lib.install Dir["lib/*"]
    share.install Dir["share/*"]
    
    # Install shell completions
    bash_completion.install "completions/seen.bash"
    zsh_completion.install "completions/_seen"
    fish_completion.install "completions/seen.fish"
  end
  
  def caveats
    <<~EOS
      Seen has been installed successfully!
      
      To get started with your first project:
        seen init hello-world
        cd hello-world
        seen build
      
      For VS Code support:
        code --install-extension seen-lang.seen-vscode
      
      Documentation: https://docs.seen-lang.org
    EOS
  end
  
  test do
    system "#{bin}/seen", "--version"
    
    # Test compilation
    (testpath/"hello.seen").write <<~EOS
      fun main() {
          println("Hello, World!")
      }
    EOS
    
    system "#{bin}/seen", "build", testpath/"hello.seen"
    assert_predicate testpath/"hello", :exist?
  end
end
```

### Scoop Manifest (Windows)

```json
{
    "version": "1.0.0",
    "description": "High-performance multi-paradigm programming language",
    "homepage": "https://seen-lang.org",
    "license": "MIT",
    "architecture": {
        "64bit": {
            "url": "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-windows-x64.zip",
            "hash": "YOUR_SHA256_HERE",
            "extract_dir": "seen-1.0.0-windows-x64"
        },
        "arm64": {
            "url": "https://github.com/seen-lang/seen/releases/download/v1.0.0/seen-1.0.0-windows-arm64.zip",
            "hash": "YOUR_SHA256_HERE",
            "extract_dir": "seen-1.0.0-windows-arm64"
        }
    },
    "bin": [
        "seen.exe",
        "seen-lsp.exe"
    ],
    "env_add_path": "bin",
    "persist": "config",
    "checkver": {
        "github": "https://github.com/seen-lang/seen"
    },
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "https://github.com/seen-lang/seen/releases/download/v$version/seen-$version-windows-x64.zip",
                "extract_dir": "seen-$version-windows-x64"
            },
            "arm64": {
                "url": "https://github.com/seen-lang/seen/releases/download/v$version/seen-$version-windows-arm64.zip",
                "extract_dir": "seen-$version-windows-arm64"
            }
        }
    },
    "notes": [
        "Seen has been installed successfully!",
        "",
        "To get started:",
        "  seen init my-project",
        "  cd my-project",
        "  seen build",
        "",
        "For VS Code support:",
        "  code --install-extension seen-lang.seen-vscode"
    ]
}
```

## GitHub Actions for Release Automation

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build-and-release:
    strategy:
      matrix:
        include:
          # Native builds
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: seen-linux-x64
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            artifact: seen-linux-arm64
          - os: ubuntu-latest
            target: riscv64gc-unknown-linux-gnu
            artifact: seen-linux-riscv64
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: seen-windows-x64
          - os: windows-latest
            target: aarch64-pc-windows-msvc
            artifact: seen-windows-arm64
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: seen-darwin-x64
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: seen-darwin-arm64
    
    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Seen toolchain
        run: |
          # Use bootstrap compiler for initial builds
          ./bootstrap.sh
      
      - name: Build for target
        run: |
          seen build --release --target ${{ matrix.target }}
      
      - name: Package artifacts
        run: |
          mkdir -p dist/${{ matrix.artifact }}
          cp target/${{ matrix.target }}/release/seen* dist/${{ matrix.artifact }}/
          cp -r stdlib dist/${{ matrix.artifact }}/
          cp -r languages dist/${{ matrix.artifact }}/
          
          # Create archive
          if [[ "${{ matrix.os }}" == "windows-latest" ]]; then
            7z a -tzip ${{ matrix.artifact }}.zip dist/${{ matrix.artifact }}/*
          else
            tar -czf ${{ matrix.artifact }}.tar.gz -C dist ${{ matrix.artifact }}
          fi
      
      - name: Build Windows MSI (Windows only)
        if: matrix.os == 'windows-latest'
        run: |
          # Install WiX toolset
          choco install wixtoolset
          
          # Build MSI
          candle.exe installer/windows/seen.wxs -dBuildDir=dist/${{ matrix.artifact }}
          light.exe -ext WixUIExtension seen.wixobj -o ${{ matrix.artifact }}.msi
      
      - name: Build Linux packages (Linux only)
        if: matrix.os == 'ubuntu-latest'
        run: |
          # Build DEB package
          ./installer/linux/build-deb.sh dist/${{ matrix.artifact }}
          
          # Build RPM package
          ./installer/linux/build-rpm.sh dist/${{ matrix.artifact }}
          
          # Build AppImage
          ./installer/linux/build-appimage.sh dist/${{ matrix.artifact }}
      
      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.artifact }}
          path: |
            ${{ matrix.artifact }}.*
            *.deb
            *.rpm
            *.AppImage
            *.msi
      
      - name: Create Release
        if: startsWith(github.ref, 'refs/tags/')
        uses: softprops/action-gh-release@v1
        with:
          files: |
            ${{ matrix.artifact }}.*
            *.deb
            *.rpm
            *.AppImage
            *.msi
          draft: false
          prerelease: false
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  
  update-package-managers:
    needs: build-and-release
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/')
    
    steps:
      - name: Update Homebrew formula
        run: |
          # Update formula with new version and SHA256
          # Submit PR to homebrew-core
      
      - name: Update Scoop manifest
        run: |
          # Update manifest with new version
          # Submit PR to scoop-extras
      
      - name: Publish to crates.io (bootstrap only)
        run: |
          cargo publish -p seen-bootstrap
      
      - name: Update Linux repositories
        run: |
          # Update APT repository
          # Update YUM repository
          # Update AUR package
```

## Testing the Installer

```bash
#!/bin/bash
# test/test_installer.sh

set -e

# Test matrix
PLATFORMS=("linux-x64" "linux-arm64" "linux-riscv64" "windows-x64" "darwin-x64")
INSTALL_METHODS=("script" "package" "docker")

for platform in "${PLATFORMS[@]}"; do
    for method in "${INSTALL_METHODS[@]}"; do
        echo "Testing $platform with $method installation..."
        
        case $method in
            script)
                # Test script installation
                docker run --rm -v $(pwd):/app $platform /app/installer/test-script.sh
                ;;
            package)
                # Test native package
                docker run --rm -v $(pwd):/app $platform /app/installer/test-package.sh
                ;;
            docker)
                # Test Docker image
                docker build -t seen-test:$platform -f installer/docker/Dockerfile.$platform .
                docker run --rm seen-test:$platform seen --version
                ;;
        esac
    done
done

echo "All installer tests passed!"
```

This comprehensive installer system provides:

1. **Native installers** for professional deployment
2. **One-line installation** for quick setup
3. **Package manager integration** for ecosystem compatibility
4. **Cross-platform support** including RISC-V
5. **Automated release pipeline** via GitHub Actions
6. **Verification and testing** infrastructure

The installer handles all dependencies, PATH configuration, and file associations automatically, making Seen as easy to install as any mainstream language.