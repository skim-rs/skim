# -*- mode: ruby -*-
# vi: set ft=ruby :
#
# Windows development VM for testing skim on Windows without a GUI.
#
# Prerequisites (host, NixOS):
#   virtualisation.libvirtd.enable = true;
#   users.users.<you>.extraGroups = [ "libvirtd" ];  # then log out/in
#
# Usage:
#   vagrant up                              # First boot: downloads box, provisions (~15-20 min)
#   vagrant up --provision                  # Re-run provisioning on existing VM
#   vagrant ssh                             # SSH in via vagrant
#   vagrant ssh-config                      # Show IP/key if you prefer a raw ssh command
#   vagrant halt                            # Stop the VM
#   vagrant destroy                         # Delete the VM
#
# Inside the VM:
#   cd C:\vagrant                           # Project root (synced from host, see note below)
#   cargo build                             # Build skim
#   cargo test                              # Run tests
#
# Note: The first `vagrant up` requires internet access on the VM to install
# packages via Chocolatey.

Vagrant.configure("2") do |config|
  # Windows Server 2022 Core — minimal footprint, no desktop GUI.
  # Box source: https://app.vagrantup.com/gusztavvargadr/boxes/windows-server-2022-standard-core
  config.vm.box = "gusztavvargadr/windows-server-2022-standard-core"

  # Vagrant manages the VM via WinRM (the Windows default).
  config.vm.communicator    = "winrm"
  config.winrm.username     = "vagrant"
  config.winrm.password     = "vagrant"
  config.winrm.timeout      = 600   # provisioning can take a while on first boot

  config.vm.provider "libvirt" do |lv|
    lv.driver = "kvm"
    lv.memory = 2048
    lv.cpus   = 2
  end

  # rsync is used for the synced folder because libvirt has no native
  # shared-folder support for Windows guests. rsync must be present on the
  # guest, so the folder is disabled on boot and synced via a post-provision
  # trigger (after Chocolatey installs rsync below).
  # Re-sync manually at any time with: vagrant rsync
  # rsync is used because libvirt has no native shared-folder support for
  # Windows guests. On a brand-new VM the very first `vagrant up` will fail
  # the rsync step (rsync not yet installed on the guest); run
  # `vagrant provision && vagrant rsync` to recover, or just
  # `vagrant destroy && vagrant up` after the box is cached locally.
  # cwRsync (the Windows rsync from Chocolatey) uses Cygwin paths, so the
  # guest path must use /cygdrive/c/... rather than a bare /vagrant.
  config.vm.synced_folder ".", "/cygdrive/c/vagrant", type: "rsync",
    rsync__exclude: [".git/", "target/", ".jj/"],
    rsync__args: ["--verbose", "--archive", "--delete", "--copy-links", "--no-owner", "--no-group"]

  # ---------------------------------------------------------------------------
  # Provisioning: configure OpenSSH + install Rust toolchain via Chocolatey.
  # The box ships with Win32-OpenSSH already present, so we only configure it.
  # Runs once on `vagrant up`; re-run with `vagrant provision`.
  # ---------------------------------------------------------------------------
  config.vm.provision "shell", privileged: true, inline: <<-'POWERSHELL'
    $ErrorActionPreference = "Stop"

    # --- Chocolatey -------------------------------------------------------------
    Write-Host "==> Installing Chocolatey..."
    if (-not (Get-Command choco -ErrorAction SilentlyContinue)) {
      Set-ExecutionPolicy Bypass -Scope Process -Force
      [System.Net.ServicePointManager]::SecurityProtocol =
        [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
      Invoke-Expression (
        (New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1')
      )
    }

    # --- OpenSSH Server ---------------------------------------------------------
    # The box ships with Win32-OpenSSH binaries at C:\Program Files\OpenSSH-Win64.
    # Re-run install-sshd.ps1 to register the service (idempotent; safe to
    # re-run if the service is already present).
    Write-Host "==> Registering and starting sshd..."
    & "C:\Program Files\OpenSSH-Win64\install-sshd.ps1"
    Set-Service  -Name sshd -StartupType Automatic
    Start-Service -Name sshd

    # Use PowerShell as the default shell for SSH sessions.
    $regPath = "HKLM:\SOFTWARE\OpenSSH"
    if (-not (Test-Path $regPath)) { New-Item -Path $regPath -Force | Out-Null }
    Set-ItemProperty -Path $regPath -Name DefaultShell `
      -Value "C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"

    # Allow inbound SSH through the Windows firewall.
    $rule = Get-NetFirewallRule -Name "OpenSSH-Server-In-TCP" -ErrorAction SilentlyContinue
    if (-not $rule) {
      New-NetFirewallRule -Name "OpenSSH-Server-In-TCP" `
        -DisplayName "OpenSSH Server (sshd)" `
        -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22
    }

    # --- Rust, Git, rsync -------------------------------------------------------
    Write-Host "==> Installing Rust, Git, rsync, and MinGW..."
    choco install -y rust git rsync mingw

    $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") +
                ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")

    # Add MinGW bin to the persistent system PATH so dlltool.exe is found in
    # SSH sessions (which don't run the Chocolatey shim refresh).
    $mingwBin = "C:\ProgramData\mingw64\mingw64\bin"
    $machinePath = [System.Environment]::GetEnvironmentVariable("Path", "Machine")
    if ($machinePath -notlike "*$mingwBin*") {
      [System.Environment]::SetEnvironmentVariable("Path", "$machinePath;$mingwBin", "Machine")
    }

    Write-Host ""
    Write-Host "==> Provisioning complete."
    Write-Host "    SSH into the VM:  vagrant ssh-config  (then ssh to the reported IP)"
    Write-Host "    Build skim:       cd C:\vagrant && cargo build"
  POWERSHELL
end
