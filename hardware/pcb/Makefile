# PetFilter PCB Build System
# Runs KiBot inside Docker for reproducible outputs
#
# Usage:
#   make validate     - Run ERC + DRC checks
#   make bom          - Generate BOM (CSV, HTML, Interactive)
#   make gerbers      - Export Gerbers + drill files (JLCPCB-ready)
#   make assembly     - Generate JLCPCB assembly files (BOM + CPL)
#   make docs         - Export schematic PDF, PCB drawings
#   make renders      - Generate 3D renders
#   make all          - Full pipeline
#   make clean        - Remove output directory
#   make shell        - Interactive Docker shell

PROJECT     := petfilter
SCHEMATIC   := $(PROJECT).kicad_sch
PCB         := $(PROJECT).kicad_pcb
KIBOT_CFG   := kibot.yaml
OUTPUT_DIR  := output
DOCKER_IMG  := ghcr.io/inti-cmnb/kicad8_auto:latest
DOCKER_RUN  := docker run --rm -v $(CURDIR):/workspace -w /workspace $(DOCKER_IMG)

.PHONY: all validate bom gerbers assembly docs renders clean shell help

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'

all: validate bom gerbers assembly docs ## Run full pipeline

validate: ## Run ERC + DRC checks
	@echo "=== Running ERC + DRC ==="
	$(DOCKER_RUN) kibot -c $(KIBOT_CFG) -s run_erc,run_drc -i
	@echo "=== Checks complete. See $(OUTPUT_DIR)/design/ ==="

bom: ## Generate BOM outputs
	@echo "=== Generating BOM ==="
	$(DOCKER_RUN) kibot -c $(KIBOT_CFG) -s all -i bom_csv bom_html interactive_bom
	@echo "=== BOM generated. See $(OUTPUT_DIR)/bom/ ==="

gerbers: ## Export Gerbers for JLCPCB
	@echo "=== Exporting Gerbers ==="
	$(DOCKER_RUN) kibot -c $(KIBOT_CFG) -s all -i gerbers drill_files gerber_zip
	@echo "=== Gerbers exported. See $(OUTPUT_DIR)/gerbers/ ==="

assembly: ## Generate JLCPCB assembly files
	@echo "=== Generating assembly files ==="
	$(DOCKER_RUN) kibot -c $(KIBOT_CFG) -s all -i position_file jlcpcb_bom
	@echo "=== Assembly files generated. See $(OUTPUT_DIR)/assembly/ ==="

docs: ## Export documentation (schematic PDF, PCB drawings)
	@echo "=== Generating documentation ==="
	$(DOCKER_RUN) kibot -c $(KIBOT_CFG) -s all -i schematic_pdf schematic_svg pcb_top_pdf
	@echo "=== Documentation generated. See $(OUTPUT_DIR)/ ==="

renders: ## Generate 3D renders
	@echo "=== Generating 3D renders ==="
	$(DOCKER_RUN) kibot -c $(KIBOT_CFG) -s all -i pcb_3d_top pcb_3d_perspective
	@echo "=== Renders generated. See $(OUTPUT_DIR)/renders/ ==="

netlist: ## Export netlist
	$(DOCKER_RUN) kibot -c $(KIBOT_CFG) -s all -i netlist

clean: ## Remove all generated outputs
	rm -rf $(OUTPUT_DIR)
	@echo "=== Output directory cleaned ==="

shell: ## Open interactive Docker shell
	docker run --rm -it -v $(CURDIR):/workspace -w /workspace $(DOCKER_IMG) bash

# Quick check: verify Docker image is available
check-docker: ## Verify Docker setup
	@docker run --rm $(DOCKER_IMG) kibot --version
	@echo "Docker KiCad environment ready."
