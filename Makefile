######## Build Flags ########
ELUSIV_NET ?=
ELUSIV_FLAGS ?=

ifeq ($(ELUSIV_NET),$(ELUSIV_FLAGS))
Elusiv_Rust_Feature_Flags := 
else
Elusiv_Rust_Feature_Flags := --features $(ELUSIV_NET) $(ELUSIV_FLAGS)
endif

Elusiv_Manifest_Path := elusiv/Cargo.toml
Elusiv_Warden_Network_Manifest_Path := elusiv-warden-network/Cargo.toml
Out_Dir := lib

Elusiv_Lib := $(Out_Dir)/elusiv.so
Elusiv_Warden_Network_Lib := $(Out_Dir)/elusiv_warden_network.so

######## Elusiv Objects ########
all: $(Elusiv_Lib)
$(Elusiv_Lib):
	@cargo build-bpf --manifest-path=$(Elusiv_Manifest_Path) --bpf-out-dir=$(Out_Dir) $(Elusiv_Rust_Feature_Flags)
	@echo "\033[1mGEN  =>  $(Elusiv_Lib)\033[0m"

######## Elusiv Warden Network ########
all: $(Elusiv_Warden_Network_Lib)
$(Elusiv_Warden_Network_Lib):
	@cargo build-bpf --manifest-path=$(Elusiv_Warden_Network_Manifest_Path) --bpf-out-dir=$(Out_Dir) $(Elusiv_Rust_Feature_Flags)
	@echo "\033[1mGEN  =>  $(Elusiv_Warden_Network_Lib)\033[0m"

.PHONY: clean-lib
clean-lib:
	@rm -f $(Elusiv_Lib)
	@rm -f $(Elusiv_Warden_Network_Lib)

.PHNOY: clean
clean:
	@rm -rf elusiv/target && rm -f elusiv/Cargo.lock
	@rm -rf elusiv-warden-network/target && rm -f elusiv-warden-network/Cargo.lock

######## Testing ########
TEST_MANIFEST ?= elusiv
TEST_METHOD ?= test # test, test-bpf, tarpaulin
TEST_FLAGS ?=

.PHNOY: test
test:
	@cd $(TEST_MANIFEST) && cargo $(TEST_METHOD) $(TEST_FLAGS)