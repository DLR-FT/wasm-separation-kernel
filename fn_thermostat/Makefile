CC = clang
CFLAGS = --target=wasm32 --no-standard-libraries -Wl,--export-all -Wl,--no-entry

all: build/main.wasm build/main.wat compile_commands.json

.PHONY: clean fmt

build:
	@mkdir --parent -- build

compile_commands.json:
	bear -- make build

clean:
	@rm --recursive --force -- build compile_commands.json

fmt:
	clang-format -i src/*


build/%.wasm: src/%.c build
	$(CC) $(CFLAGS) $< -o $@

	
build/%.wat: build/%.wasm build
	wasm2wat $< -o $@
