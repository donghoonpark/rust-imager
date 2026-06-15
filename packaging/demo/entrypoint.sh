#!/usr/bin/env bash
set -euo pipefail

work_dir=/run/rust-imager-demo
disk_image="$work_dir/emmc.img"
target_image="$work_dir/target-emmc.img"
mkdir -p "$work_dir" /output

truncate -s 768M "$disk_image"
printf 'label: dos\nunit: sectors\n\nstart=2048, size=131072, type=c, bootable\nstart=133120, type=83\n' |
    sfdisk "$disk_image" >/dev/null

loop_device=$(losetup --find --show --partscan "$disk_image")
loop_name=${loop_device#/dev/}
truncate -s 1G "$target_image"
target_loop_device=$(losetup --find --show "$target_image")
target_loop_name=${target_loop_device#/dev/}

cleanup() {
    umount "$work_dir/root" 2>/dev/null || true
    umount "$work_dir/boot" 2>/dev/null || true
    losetup -d "$loop_device" 2>/dev/null || true
    losetup -d "$target_loop_device" 2>/dev/null || true
}
trap cleanup EXIT

for partition in 1 2; do
    node="${loop_device}p${partition}"
    sys_dev="/sys/class/block/${loop_name}p${partition}/dev"
    if [[ ! -b "$node" ]]; then
        IFS=: read -r major minor <"$sys_dev"
        mknod "$node" b "$major" "$minor"
    fi
done

create_alias_node() {
    local target_sys_dev=$1
    local alias_path=$2
    local major minor
    IFS=: read -r major minor <"$target_sys_dev"
    rm -f "$alias_path"
    mknod "$alias_path" b "$major" "$minor"
}

create_alias_node "/sys/class/block/$loop_name/dev" /dev/sdz
create_alias_node "/sys/class/block/${loop_name}p1/dev" /dev/sdz1
create_alias_node "/sys/class/block/${loop_name}p2/dev" /dev/sdz2
create_alias_node "/sys/class/block/$target_loop_name/dev" /dev/sdy

mkfs.vfat -F 32 -n BOOT "${loop_device}p1" >/dev/null
mkfs.ext4 -F -L rootfs "${loop_device}p2" >/dev/null

mkdir -p "$work_dir/boot" "$work_dir/root"
mount "${loop_device}p1" "$work_dir/boot"
mount "${loop_device}p2" "$work_dir/root"
mkdir -p "$work_dir/root"/{etc,usr,var,boot}
printf 'rust-imager demo boot payload\n' >"$work_dir/boot/config.txt"
printf 'rust-imager full extraction payload\n' >"$work_dir/root/etc/demo-release"
dd if=/dev/zero of="$work_dir/root/var/demo-padding.bin" bs=1M count=48 status=none
sync
umount "$work_dir/root"
umount "$work_dir/boot"

real_lsblk=$(command -v lsblk)
cat >"$work_dir/lsblk" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [[ " \$* " == *" --bytes "* ]]; then
"$real_lsblk" "\$@" >/dev/null
cat <<'JSON'
{"blockdevices":[
  {"name":"/dev/nvme0n1","type":"disk","size":1099511627776,"log-sec":512,"tran":"nvme","rm":false,"model":"System Disk","serial":"SYSTEM","maj:min":"259:0"},
  {"name":"/dev/sdy","type":"disk","size":1073741824,"log-sec":512,"tran":"usb","rm":true,"model":"Virtual Flash Target","serial":"DEMO-TARGET-001","maj:min":"$(<"/sys/class/block/$target_loop_name/dev")"},
  {"name":"/dev/sdz","type":"disk","size":805306368,"log-sec":512,"tran":"usb","rm":true,"model":"Virtual eMMC Reader","serial":"DEMO-EMMC-001","maj:min":"$(<"/sys/class/block/$loop_name/dev")",
   "children":[
    {"name":"/dev/sdz1","type":"part","size":67108864,"log-sec":512,"fstype":"vfat"},
    {"name":"/dev/sdz2","type":"part","size":736100352,"log-sec":512,"fstype":"ext4"}
   ]}
]}
JSON
else
"$real_lsblk" "\$@" >/dev/null
if [[ " \$* " == *" /dev/sdy "* ]]; then
cat <<'JSON'
{"blockdevices":[{"name":"/dev/$target_loop_name","mountpoint":null}]}
JSON
else
cat <<'JSON'
{"blockdevices":[
  {"name":"/dev/$loop_name","mountpoint":null,
   "children":[
    {"name":"/dev/${loop_name}p1","mountpoint":null},
    {"name":"/dev/${loop_name}p2","mountpoint":null}
   ]}
]}
JSON
fi
fi
EOF
chmod 0755 "$work_dir/lsblk"

real_partprobe=$(command -v partprobe)
cat >"$work_dir/partprobe" <<EOF
#!/usr/bin/env bash
set -euo pipefail
"$real_partprobe" "\$@"
if [[ " \$* " == *" /dev/sdy "* ]]; then
    exit 0
fi
for partition in 1 2; do
    sys_dev="/sys/class/block/${loop_name}p\${partition}/dev"
    for _ in {1..50}; do
        [[ -r "\$sys_dev" ]] && break
        sleep 0.02
    done
    IFS=: read -r major minor <"\$sys_dev"
    rm -f "/dev/sdz\${partition}"
    mknod "/dev/sdz\${partition}" b "\$major" "\$minor"
done
EOF
chmod 0755 "$work_dir/partprobe"

if [[ ${RUST_IMAGER_DEMO_TRACE:-0} == 1 ]]; then
    real_e2fsck=$(command -v e2fsck)
    cat >"$work_dir/e2fsck" <<EOF
#!/usr/bin/env bash
exec "$real_e2fsck" "\$@" 1>&2
EOF
    chmod 0755 "$work_dir/e2fsck"
fi

export PATH="$work_dir:$PATH"
export RUST_BACKTRACE=1

if [[ ${1:-} == shell ]]; then
    bash
    exit
fi

if [[ ${1:-} == full-run ]]; then
    /usr/local/lib/rust-imager-demo/full-run.sh
    exit
fi

rust-imager "$@"
