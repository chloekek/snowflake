let
    nixpkgs = import common/nix/nixpkgs;

    rustChannel = nixpkgs.rustChannelOf {
        date = "2022-05-01";
        channel = "nightly";
        sha256 = "0yshryfh3n0fmsblna712bqgcra53q3wp1asznma1sf6iqrkrl02";
    };
in
    nixpkgs.mkShell {

        # Tools available in Nix shell.
        nativeBuildInputs = [
            nixpkgs.cacert
            nixpkgs.python3Packages.sphinx
            rustChannel.rust
        ];

        SNOWFLAKE_BASH      = nixpkgs.bash;
        SNOWFLAKE_COREUTILS = nixpkgs.coreutils;
        SNOWFLAKE_GNUM4     = nixpkgs.gnum4;
        SNOWFLAKE_MINIFY    = nixpkgs.minify;
        SNOWFLAKE_SASSC     = nixpkgs.sassc;

    }
