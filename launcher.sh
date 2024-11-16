macOS_version=$(sw_vers -productVersion)
arch=$(uname -m)

case "$macOS_version" in
15*)
    macOS_version=sequoia
    ;;
14*)
    macOS_version=sonoma
    ;;
13*)
    macOS_version=ventura
    ;;
12*)
    macOS_version=monterey
    ;;
11*)
    macOS_version=bigsur
    ;;
10.15*)
    macOS_version=catalina
    ;;
10.14*)
    macOS_version=mojave
    ;;
10.13*)
    macOS_version=high_sierra
    ;;
esac

if [ "$arch" = "arm64" ]; then
    macOS_version=arm64_$macOS_version
    link="https://github.com/hyperpuncher/mac-soft-rs/releases/latest/download/mac-soft-rs_arm64"
else
    link="https://github.com/hyperpuncher/mac-soft-rs/releases/latest/download/mac-soft-rs"
fi

temp_file=$(mktemp)
chmod +x "$temp_file"
curl -sL "$link" -o "$temp_file"

"$temp_file" --macos-version "$macOS_version"
