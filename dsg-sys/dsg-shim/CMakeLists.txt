# dsg-shim/CMakeLists.txt
cmake_minimum_required(VERSION 3.18)
project(dsg_c_api LANGUAGES C CXX)

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(CMAKE_RULE_MESSAGES ON)
set(FETCHCONTENT_QUIET OFF)

# Options to control behavior from the outside (or default to ON)
option(BUILD_SPARK_DSG_FROM_SOURCE "Fetch & build Spark-DSG from source if not found" ON)

# If a user points to a local install, use it.
# e.g., cargo: CMAKE_PREFIX_PATH=/path/to/sdsg-install
find_package(SparkDSG QUIET)

if(NOT SparkDSG_FOUND AND BUILD_SPARK_DSG_FROM_SOURCE)
  include(FetchContent)
  # Pin a known good ref if you want reproducible builds
  FetchContent_Declare(
    SparkDSG
    GIT_REPOSITORY https://github.com/MIT-SPARK/Spark-DSG.git
    GIT_TAG main
  )

  # Propagate Spark-DSG build options
  set(SPARK_DSG_BUILD_PYTHON OFF CACHE BOOL "" FORCE)
  set(SPARK_DSG_BUILD_TESTS  OFF CACHE BOOL "" FORCE)
  set(SPARK_DSG_BUILD_ZMQ    ON  CACHE BOOL "" FORCE)

  # You can also force RelWithDebInfo/Release
  if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "" FORCE)
  endif()

  FetchContent_MakeAvailable(SparkDSG)
endif()

# ---- Your C shim library ----

# find_package(spark_dsg REQUIRED)  # exported by Spark-DSG install
add_library(dsg_c_api SHARED dsg_c_api.cpp)
target_link_libraries(dsg_c_api PUBLIC spark_dsg::spark_dsg) # adjust if target is named differently

target_include_directories(dsg_c_api PUBLIC
  $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>
  $<INSTALL_INTERFACE:include>
)

set_target_properties(dsg_c_api PROPERTIES
  C_STANDARD 99
  CXX_STANDARD 17
  POSITION_INDEPENDENT_CODE ON
)

install(TARGETS dsg_c_api
  LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
  ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
  RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)
