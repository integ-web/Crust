{
  description = "Crust-RustyAgent Autonomous OS";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay, ... }:
    let
      system = "x86_64-linux";
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs { inherit system overlays; };

      # 1. Package the Crust-RustyAgent Rust workspace
      agentPackage = pkgs.rustPlatform.buildRustPackage {
        pname = "crust-rustyagent";
        version = "0.1.0";

        src = ./.;

        cargoLock = {
          lockFile = ./Cargo.lock;
          # For workspaces, Cargo.lock might need to be explicitly handled
          allowBuiltinFetchGit = true;
        };

        nativeBuildInputs = with pkgs; [
          pkg-config
          rust-bin.stable.latest.default
        ];

        buildInputs = with pkgs; [
          openssl
          sqlite
          chromium
          fontconfig
          freetype
          # Dependencies often required by chromiumoxide or headless browsers
          xorg.libX11
          xorg.libXcomposite
          xorg.libXcursor
          xorg.libXdamage
          xorg.libXext
          xorg.libXi
          xorg.libXrender
          xorg.libXtst
          xorg.libXrandr
          xorg.libxcb
          glib
          nss
          nspr
          atk
          at-spi2-atk
          cups
          dbus
          expat
          libdrm
          mesa
          alsa-lib
          pango
          cairo
        ];

        # Configure environment variables during build
        PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
      };

    in {
      # This builds the Rust project as a standard package (nix build)
      packages.${system}.default = agentPackage;

      # 2. Define the NixOS Live-Boot ISO Configuration
      nixosConfigurations.liveIso = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          # Import the standard NixOS installation CD/ISO profile
          "${nixpkgs}/nixos/modules/installer/cd-dvd/installation-cd-minimal.nix"

          ({ pkgs, ... }: {
            # --- System and Hardware Configuration ---
            nixpkgs.config.allowUnfree = true;

            # --- Immutability: Stateless Boot Strategy ---
            # By default, the ISO profile uses a tmpfs (RAM disk) for `/` which provides
            # boot-time immutability out of the box. Changes are wiped on reboot.

            # Include the Rust Agent package in the system environment
            environment.systemPackages = [
              agentPackage
              pkgs.chromium
              pkgs.sqlite
              pkgs.git
            ];

            # Ensure Chromium can run headless in the live environment
            environment.sessionVariables = {
              CHROME_EXECUTABLE = "${pkgs.chromium}/bin/chromium";
              PUPPETEER_SKIP_CHROMIUM_DOWNLOAD = "true";
              RUST_LOG = "info";
            };

            # --- Networking ---
            networking.hostName = "rustyagent-os";
            networking.networkmanager.enable = true;
            networking.wireless.enable = false; # Handled by NetworkManager

            # --- Autostart the Agent as an OS Service ---
            # We configure a systemd service to run the 'cli' binary on tty1
            # effectively acting as the OS user interface.
            systemd.services.rustyagent = {
              description = "Crust-RustyAgent Autonomous Service";
              wantedBy = [ "multi-user.target" ];
              after = [ "network-online.target" ];
              wants = [ "network-online.target" ];

              serviceConfig = {
                Type = "simple";
                ExecStart = "${agentPackage}/bin/cli";
                Restart = "always";
                RestartSec = 5;
                StandardOutput = "tty";
                StandardError = "journal";
                TTYPath = "/dev/tty1";
                TTYReset = "yes";
                TTYVHangup = "yes";
                # Run as root for hardware probing access (sysinfo/nvml) in the live CD
                User = "root";
              };
            };

            # Disable the default getty on tty1 so our agent can take over the screen
            systemd.services."getty@tty1".enable = false;
          })
        ];
      };
    };
}
