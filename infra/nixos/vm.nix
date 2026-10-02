{
  config,
  lib,
  pkgs,
  modulesPath,
  sandbox,
  ...
}:
let
  policy = sandbox.m0;
  probe = pkgs.writeShellApplication {
    name = "warden-preflight";
    runtimeInputs = with pkgs; [
      bpftools
      python3
      systemd
      util-linux
      coreutils
    ];
    text = ''
      exec python3 ${../../tests/vm/preflight.py}
    '';
  };
in
{
  imports = [ (modulesPath + "/virtualisation/qemu-vm.nix") ];
  system.stateVersion = "26.05";
  networking.hostName = "veil-warden-sandbox";
  boot.kernelPackages = pkgs.linuxPackages_6_18;
  boot.kernel.sysctl."kernel.unprivileged_bpf_disabled" = 1;
  boot.kernelParams = [ "systemd.unified_cgroup_hierarchy=1" ];

  nix.settings.experimental-features = [
    "nix-command"
    "flakes"
  ];
  nix.channel.enable = false;
  networking.firewall.enable = true;
  networking.nameservers = [ "1.1.1.1" ];
  services.openssh = {
    enable = true;
    settings = {
      PermitRootLogin = "no";
      PasswordAuthentication = false;
      KbdInteractiveAuthentication = false;
      AllowUsers = [ "warden" ];
    };
    authorizedKeysFiles = [ "/var/lib/warden-keys/operator.pub" ];
  };
  users.mutableUsers = false;
  users.users.warden = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];
    # Local console is deliberately passwordless, matching console autologin.
    # Remote password authentication remains disabled.
    hashedPassword = "";
  };
  security.sudo.wheelNeedsPassword = false;
  services.getty.autologinUser = "warden";

  systemd.slices."warden-test" = {
    description = "Only synthetic veil-warden test clients";
    sliceConfig = {
      CPUQuota = "100%";
      MemoryMax = "512M";
      TasksMax = 64;
    };
  };
  environment.systemPackages = with pkgs; [
    probe
    bpftools
    iproute2
    python3
    git
    rustc
    cargo
  ];
  virtualisation = {
    cores = policy.cpus;
    memorySize = policy.memory_mib;
    diskImage = null;
    graphics = false;
    useNixStoreImage = true;
    mountHostNixStore = false;
    useHostCerts = false;
    writableStore = true;
    writableStoreUseTmpfs = true;
    sharedDirectories = lib.mkForce {
      keys = {
        source = ''"$WARDEN_KEYS_DIR"'';
        target = "/var/lib/warden-keys";
        securityModel = "none";
      };
    };
    forwardPorts = [
      {
        from = "host";
        host.address = policy.ssh_bind;
        host.port = policy.ssh_port;
        guest.port = 22;
      }
    ];
  };
  assertions = [
    {
      assertion = policy.mode == "observe" && !policy.attach_enabled;
      message = "M0 must not attach or enforce eBPF policy.";
    }
    {
      assertion = policy.scope == "/warden.slice/warden-test.slice";
      message = "Only the dedicated test slice is permitted.";
    }
    {
      assertion = policy.ssh_bind == "127.0.0.1";
      message = "Management SSH must bind to host loopback.";
    }
    {
      assertion =
        !config.virtualisation.mountHostNixStore
        && !config.virtualisation.useHostCerts
        && builtins.attrNames config.virtualisation.sharedDirectories == [ "keys" ];
      message = "Only the runtime public-key directory may be shared.";
    }
    {
      assertion = config.virtualisation.diskImage == null;
      message = "M0 sandbox root is ephemeral; recovery must not require deleting disks.";
    }
    {
      assertion =
        config.services.openssh.settings.PermitRootLogin == "no"
        && !config.services.openssh.settings.PasswordAuthentication;
      message = "SSH must use the dedicated non-root user and public keys.";
    }
  ];
}
