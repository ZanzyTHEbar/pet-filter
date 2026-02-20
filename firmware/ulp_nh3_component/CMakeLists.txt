cmake_minimum_required(VERSION 3.16)

# ULP component wrapper: build and embed the NH3 monitor ULP-RISC-V binary.
idf_component_register(SRCS "ulp_exports.c" INCLUDE_DIRS "." REQUIRES ulp)

set(ulp_app_name ulp_nh3_monitor)
set(ulp_riscv_sources "ulp_nh3_monitor.c")
set(ulp_exp_dep_srcs "ulp_exports.c")

ulp_embed_binary(${ulp_app_name} "${ulp_riscv_sources}" "${ulp_exp_dep_srcs}")
