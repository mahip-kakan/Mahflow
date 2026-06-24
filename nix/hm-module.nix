# Home-manager module for Mahflow speech-to-text
#
# Provides a systemd user service for autostart.
# Usage: imports = [ mahflow.homeManagerModules.default ];
#        services.mahflow.enable = true;
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.mahflow;
in
{
  options.services.handy = {
    enable = lib.mkEnableOption "Mahflow speech-to-text user service";

    package = lib.mkOption {
      type = lib.types.package;
      defaultText = lib.literalExpression "mahflow.packages.\${system}.mahflow";
      description = "The Mahflow package to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.handy = {
      Unit = {
        Description = "Mahflow speech-to-text";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = "${cfg.package}/bin/mahflow";
        Restart = "on-failure";
        RestartSec = 5;
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
