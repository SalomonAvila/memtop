#!/usr/bin/env sh
# Regenera data/pci-gpu.ids: un extracto de la base oficial pci.ids
# (https://pci-ids.ucw.cz) con solo los fabricantes de GPU que reconoce memtop.
#
# Uso:
#   scripts/actualizar-pci-ids.sh                 # descarga la última versión
#   scripts/actualizar-pci-ids.sh /ruta/pci.ids   # usa un archivo local
#
# pci.ids se distribuye bajo GPL-2.0-or-later o BSD-3-Clause; se conserva su
# cabecera original con el aviso de licencia.
set -eu

destino="$(dirname "$0")/../data/pci-gpu.ids"
origen="${1:-}"

if [ -z "$origen" ]; then
    origen="$(mktemp)"
    trap 'rm -f "$origen"' EXIT
    curl -fsSL https://pci-ids.ucw.cz/v2.2/pci.ids -o "$origen"
fi

# Fabricantes: NVIDIA, AMD/ATI, Intel, Matrox, Cirrus, ASPEED, Qualcomm, Microsoft (Hyper-V),
# VMware, VirtualBox, Red Hat/QEMU (virtio, bochs), Moore Threads.
awk '
    BEGIN {
        split("10de 1002 1022 8086 102b 1013 1a03 5143 1414 15ad 80ee 1af4 1b36 1234 1ed5", v, " ")
        for (i in v) quiero[v[i]] = 1
        print "#\tExtracto de pci.ids para memtop: solo fabricantes de GPU, sin subsistemas."
        print "#\tGenerado con scripts/actualizar-pci-ids.sh. No editar a mano."
    }
    /^C / { exit }                       # empieza la lista de clases: ya no hay fabricantes
    NR <= 20 && /^#/ { print; next }     # cabecera original (versión y licencia)
    /^#/ || /^[[:space:]]*$/ { next }
    /^\t\t/ { next }                     # subsistemas
    /^\t/ { if (dentro) print; next }    # dispositivos
    { dentro = (substr($0, 1, 4) in quiero); if (dentro) print }
' "$origen" > "$destino"

echo "Escrito $destino ($(wc -l < "$destino") líneas)"
