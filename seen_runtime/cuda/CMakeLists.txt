cmake_minimum_required(VERSION 3.25)
project(seen_cuda LANGUAGES CXX)

option(SEEN_ENABLE_CUDA "Build the opt-in Seen CUDA native subsystem" OFF)
if(NOT SEEN_ENABLE_CUDA)
  message(FATAL_ERROR "Seen CUDA discovery is gated; pass -DSEEN_ENABLE_CUDA=ON explicitly")
endif()
if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux" OR
   NOT CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|amd64|AMD64)$")
  message(FATAL_ERROR "seen.cuda.unsupported_platform: CUDA is currently Linux x86-64 only")
endif()

enable_language(CUDA)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CUDA_STANDARD 17)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_CUDA_EXTENSIONS OFF)
set(CMAKE_CUDA_ARCHITECTURES 89 CACHE STRING "Explicit CUDA architectures")

find_package(CUDAToolkit 12.0 REQUIRED)

add_library(seen_cuda SHARED src/seen_cuda.cu)
target_include_directories(seen_cuda PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include)
target_link_libraries(seen_cuda PRIVATE CUDA::cudart CUDA::cublasLt)
target_compile_options(seen_cuda PRIVATE
  $<$<COMPILE_LANGUAGE:CXX>:-Wall;-Wextra;-Werror>
  $<$<COMPILE_LANGUAGE:CUDA>:--threads=1;--Werror=cross-execution-space-call>)
set_target_properties(seen_cuda PROPERTIES
  CXX_VISIBILITY_PRESET hidden
  CUDA_VISIBILITY_PRESET hidden
  POSITION_INDEPENDENT_CODE ON
  OUTPUT_NAME seen_cuda
  VERSION 1.0.0
  SOVERSION 1)

install(TARGETS seen_cuda LIBRARY DESTINATION lib)
install(FILES include/seen_cuda.h DESTINATION include/seen)
