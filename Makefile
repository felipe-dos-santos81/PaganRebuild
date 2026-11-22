# Makefile for PaganRebuild — engine for Ultima VIII: Pagan (Rust)
# Targets follow the working order:
#   build → run (in a window) / screenshot (without one) → test → lint
SERVICE = PaganRebuild

# Variables
CARGO = cargo
BIN = target/release/PaganRebuild
# The game data are not in the repository. The GOG version for the Mac keeps
# them inside its app; another folder is given with game="<dir>".
game ?= $(HOME)/Documents/gog/Ultima™ 8 Gold Edition.app/Contents/Resources/game
lang ?= english
scale ?= 2
do ?= t60;look:1;t30
out ?= target/shot.png
# The window is opened by SDL3; sdl=0 builds the one of winit instead.
sdl ?= 1
FEATURES = $(if $(filter 1,$(sdl)),--features sdl)
GAME_ARGS = --game "$(game)" --lang "$(lang)"

.PHONY: help install build clean \
        run screenshot \
        test test-case test-journey lint

# ── Environment ──────────────────────────────────────────────────────────────

help: ## Print this help message
	@printf '\033[01;32m${SERVICE} — engine for Ultima VIII: Pagan\033[00;37m\n\n'
	@printf "\033[33mUsage:\033[0m\n  make [target] [arg=\"val\"...]\n\n\033[33mTargets:\033[0m\n"
	@grep -E '^[-a-zA-Z0-9_\.\/]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; \
		{printf "  \033[36m%-26s\033[0m %s\n", $$1, $$2}'

install: ## Check that Rust and (for sdl=1) SDL3 are there
	@command -v $(CARGO) >/dev/null || { echo "Rust is needed: https://rustup.rs"; exit 1; }
	@$(if $(filter 1,$(sdl)),pkg-config --exists sdl3 2>/dev/null || { echo "SDL3 is needed: brew install sdl3 pkg-config"; exit 1; },true)

build: install ## Build the release program for this machine (Apple Silicon: arm64, Metal; usage: make build [sdl=0])
	$(CARGO) build --release $(FEATURES)

clean: ## Remove everything that was built (target/)
	$(CARGO) clean
	@echo "Cleanup complete."

# ── Running ──────────────────────────────────────────────────────────────────

# The game opens in a window and leaves the mouse pointer free to leave it.
run: build ## Start the game in a window (usage: make run [game="<dir>"] [lang=german] [scale=2] [window=1280x800] [nosound=1] [nomenu=1] [sdl=0] [args="..."])
	$(BIN) $(GAME_ARGS) --scale "$(scale)" $(if $(window),--window "$(window)") $(if $(nosound),--no-sound) $(if $(nomenu),--no-menu) $(args)

screenshot: build ## Render one picture without a window (usage: make screenshot [do="t60;look:1;t30"] [out=target/shot.png] [args="..."])
	$(BIN) $(GAME_ARGS) --no-sound --screenshot "$(out)" --do "$(do)" $(args)
	@echo "Picture written to $(out)."

# ── Development ──────────────────────────────────────────────────────────────

# The cases are skipped unless the repository lies inside the game folder
# (a folder above it with ENGLISH/STATIC).
test: ## Run the unit tests and all regression cases (about two minutes)
	$(CARGO) test --release $(FEATURES)

test-case: ## Run one regression case (usage: make test-case case=<name>)
	PAGAN_CASE="$(case)" $(CARGO) test --release $(FEATURES) -- --nocapture

test-journey: ## Run the playthrough of the whole game (long)
	$(CARGO) test --release $(FEATURES) --test journey -- --ignored --nocapture

lint: ## Run clippy on all targets, with SDL and without (should stay free of warnings)
	$(CARGO) clippy --all-targets --features sdl
	$(CARGO) clippy --all-targets
