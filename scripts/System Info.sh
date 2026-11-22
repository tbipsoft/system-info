#!/bin/bash
#
# System Info -- launcher do menu "Tools" do dArkOS para o app rg52-info
# (informacoes do sistema na tela, no mesmo visual do rg52-led).
#
# O app so le /proc e /sys: nao precisa de root. Onde colocar: este .sh na pasta dos
# scripts do menu Tools e o binario `rg52-info` numa destas opcoes:
#   <pasta deste script>/rg52-info/rg52-info   (pasta "rg52-info" ao lado do .sh)
#   <pasta deste script>/rg52-info             (binario ao lado do .sh)
#   /opt/rg52-info/rg52-info
#   DEVICE_PROFILE=r36s  forca o perfil (rg52mini, rg43, r36s, rgb10, a10mini, rk3566, generic)
# Para trocar o destaque: ACCENT=teal (amber, teal, violet, coral ou #RRGGBB).

APP=rg52-info
LOG=/tmp/rg52-info.log
RUN=/tmp/rg52-info.run
TTY=${TTY:-/dev/tty1}
USE_GPTOKEYB=${USE_GPTOKEYB:-0}

HERE="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"

say() { printf '\033c%s\n' "$1" > "$TTY" 2>/dev/null; sleep "${2:-5}"; }

find_bin() {
  local c
  for c in "$HERE/$APP/$APP" "$HERE/$APP" "/opt/$APP/$APP"; do
    [ -f "$c" ] && { echo "$c"; return 0; }
  done
  return 1
}

cleanup() {
  [ "$USE_GPTOKEYB" = "1" ] && pgrep -f gptokeyb | sudo xargs -r kill -9
  rm -f "$RUN"
  printf '\033c' > "$TTY" 2>/dev/null
}

sudo chmod 666 "$TTY" 2>/dev/null
printf '\033c\e[?25l' > "$TTY" 2>/dev/null

BIN=$(find_bin) || {
  say "rg52-info nao encontrado.

Coloque o binario em uma destas opcoes:
  $HERE/$APP/$APP
  $HERE/$APP
  /opt/$APP/$APP" 8
  cleanup
  exit 1
}

# Copia para /tmp: funciona mesmo com o binario numa particao FAT/noexec.
if ! cp -f "$BIN" "$RUN" || ! chmod +x "$RUN"; then
  say "Nao consegui preparar $RUN (disco cheio ou /tmp somente leitura?)" 6
  cleanup
  exit 1
fi

export SDL_GAMECONTROLLERCONFIG_FILE="/opt/inttools/gamecontrollerdb.txt"
export XDG_RUNTIME_DIR="/run/user/$UID/"

ARGS=(--fullscreen)
[ -n "$DEVICE_PROFILE" ] && ARGS+=(--device "$DEVICE_PROFILE")
[ -n "$ACCENT" ] && ARGS+=(--accent "$ACCENT")
if [ "$USE_GPTOKEYB" = "1" ]; then
  sudo chmod 666 /dev/uinput
  pgrep -f gptokeyb | sudo xargs -r kill -9
  /opt/inttools/gptokeyb -1 "$(basename "$RUN")" -c "/opt/inttools/keys.gptk" > /dev/null 2>&1 &
  ARGS+=(--no-gamepad)
fi

# Roda como o proprio usuario (o app so le /proc e /sys). O launcher do LED RGB usa
# `sudo env` porque la a serial exige root; aqui nao precisa.
env \
  SDL_GAMECONTROLLERCONFIG_FILE="$SDL_GAMECONTROLLERCONFIG_FILE" \
  XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
  ${SDL_VIDEODRIVER:+SDL_VIDEODRIVER="$SDL_VIDEODRIVER"} \
  "$RUN" "${ARGS[@]}" > "$LOG" 2>&1
STATUS=$?

if [ "$STATUS" -ne 0 ]; then
  say "rg52-info terminou com erro ($STATUS).

$(tail -n 12 "$LOG" 2>/dev/null)

Log completo: $LOG" 10
fi

cleanup
exit "$STATUS"
