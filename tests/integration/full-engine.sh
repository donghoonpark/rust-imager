#!/usr/bin/env bash
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "full-engine.sh must run as root" >&2
    exit 1
fi
if [[ -e /dev/sdz || -e /dev/sdz1 || -e /dev/sdz2 ]]; then
    echo "/dev/sdz is already in use" >&2
    exit 1
fi

work=$(mktemp -d)
image="$work/source.img"
output="$work/output.img.zst"
boot="$work/boot"
root="$work/root"
fakebin="$work/bin"
loop=
cleanup() {
    if mountpoint -q "$boot"; then
        umount "$boot"
    fi
    if mountpoint -q "$root"; then
        umount "$root"
    fi
    rm -f /dev/sdz /dev/sdz1 /dev/sdz2
    if [[ -n "$loop" ]]; then
        losetup -d "$loop" 2>/dev/null || true
    fi
    rm -rf "$work"
}
trap cleanup EXIT

truncate -s 1G "$image"
sfdisk "$image" <<'SFDISK'
label: dos
unit: sectors

start=2048, size=131072, type=c, bootable
start=133120, type=83
SFDISK
loop=$(losetup --find --show --partscan "$image")
loop_name=$(basename "$loop")
ln -s "$loop" /dev/sdz
ln -s "${loop}p1" /dev/sdz1
ln -s "${loop}p2" /dev/sdz2
mkfs.vfat -F 32 -n boot /dev/sdz1
mkfs.ext4 -F -L rootfs /dev/sdz2
mkdir "$boot" "$root" "$fakebin"
mount /dev/sdz1 "$boot"
mount /dev/sdz2 "$root"
echo fixture >"$boot/config.txt"
echo firmware >"$boot/start4.elf"
mkdir -p "$root/etc" "$root/usr" "$root/var"
echo engine-payload >"$root/etc/rust-imager-payload"
sync
umount "$boot"
umount "$root"

cat >"$fakebin/lsblk" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [[ " \$* " == *" --bytes "* ]]; then
cat <<'JSON'
{"blockdevices":[
  {"name":"/dev/nvme0n1","type":"disk","size":68719476736,"log-sec":512,"tran":"nvme","model":"SYSTEM","serial":"SYS","maj:min":"259:0","rm":false,
   "children":[{"name":"/dev/nvme0n1p1","type":"part","size":68718428160,"fstype":"ext4"}]},
  {"name":"/dev/sdz","type":"disk","size":1073741824,"log-sec":512,"tran":"usb","model":"Virtual eMMC Reader","serial":"CI-FIXTURE","maj:min":"7:250","rm":true,
   "children":[
     {"name":"/dev/sdz1","type":"part","size":67108864,"fstype":"vfat"},
     {"name":"/dev/sdz2","type":"part","size":1005584384,"fstype":"ext4"}
   ]}
]}
JSON
else
cat <<'JSON'
{"blockdevices":[{"name":"/dev/$loop_name","mountpoint":null,"children":[
  {"name":"/dev/${loop_name}p1","mountpoint":null},
  {"name":"/dev/${loop_name}p2","mountpoint":null}
]}]}
JSON
fi
EOF
chmod +x "$fakebin/lsblk"

cat >"$fakebin/findmnt" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [[ " \$* " == *" --json "* ]]; then
    printf '%s\n' '{"filesystems":[{"target":"/","source":"/dev/nvme0n1p1"}]}'
elif [[ "\${1:-}" == "-T" ]]; then
    printf '%s\n' 'ext4 /dev/nvme0n1p1'
elif [[ " \$* " == *" -S /dev/sdz2 "* ]]; then
    exec /usr/bin/findmnt -rn -S "${loop}p2" -o TARGET
else
    exec /usr/bin/findmnt "\$@"
fi
EOF
chmod +x "$fakebin/findmnt"

cargo build -p rust-imager
PATH="$fakebin:$PATH" target/debug/rust-imager image \
    --device /dev/sdz \
    --output "$output" \
    --confirm-model "Virtual eMMC Reader" \
    --compression zstd \
    --level 1 \
    --verify reread

test -s "$output"
test -s "$output.json"
test -s "$output.log"
grep -q 'result=success' "$output.log"
grep -q 'source_modified=true' "$output.log"
zstd -q -t "$output"
jq -e '.verification.status == "passed"' "$output.json" >/dev/null
jq -e '.partitions | length == 2' "$output.json" >/dev/null
echo "complete imaging engine verified"
