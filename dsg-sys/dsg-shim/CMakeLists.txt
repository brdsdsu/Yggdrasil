cmake_minimum_required(VERSION 3.16)
project(dsg_c_api LANGUAGES C CXX)
  
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)


# Let the caller pass CMAKE_PREFIX_PATH so we can find spark_dsg
find_package(spark_dsg REQUIRED)  # exported by Spark-DSG install

add_library(dsg_c_api SHARED dsg_c_api.cpp)
target_include_directories(dsg_c_api PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_link_libraries(dsg_c_api PUBLIC spark_dsg::spark_dsg) # adjust if target is named differently

# Optional: install for system-wide usage
install(TARGETS dsg_c_api EXPORT dsg_c_apiTargets
        LIBRARY DESTINATION lib
        ARCHIVE DESTINATION lib
        RUNTIME DESTINATION bin)
install(FILES dsg_c_api.h DESTINATION include)
