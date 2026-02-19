cmake_minimum_required(VERSION 3.16)

idf_component_register()

set(ulp_app_name ulp_nh3_monitor)
set(ulp_sources "ulp_nh3_monitor.c")
set(ulp_exp_dep_srcs "")

ulp_embed_binary(${ulp_app_name} "${ulp_sources}" "${ulp_exp_dep_srcs}")
