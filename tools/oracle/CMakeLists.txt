# Oracle: upstream packingsolver at the pinned commit (docs/UPSTREAM.md),
# wrapped to read and write sheetsolver's JSON formats.
#
#   cmake -S tools/oracle -B tools/oracle/build -DCMAKE_BUILD_TYPE=Release
#   cmake --build tools/oracle/build -j
cmake_minimum_required(VERSION 3.28)
project(SheetsolverOracle LANGUAGES CXX)

include(FetchContent)

# Keep in sync with docs/UPSTREAM.md.
set(PACKINGSOLVER_COMMIT 9b656a0dcafd1e9019bf39a5910d4307af65579a)

# Only the libraries: no upstream CLI (needs Boost), no tests. The tree
# search does not use an LP solver; HiGHS is still built because upstream
# fetches it unconditionally.
set(PACKINGSOLVER_BUILD_MAIN OFF CACHE BOOL "" FORCE)
set(PACKINGSOLVER_BUILD_TEST OFF CACHE BOOL "" FORCE)
set(PACKINGSOLVER_USE_CLP OFF CACHE BOOL "" FORCE)
set(PACKINGSOLVER_USE_HIGHS ON CACHE BOOL "" FORCE)

FetchContent_Declare(
    packingsolver
    GIT_REPOSITORY https://github.com/fontanf/packingsolver.git
    GIT_TAG ${PACKINGSOLVER_COMMIT}
    EXCLUDE_FROM_ALL)
FetchContent_MakeAvailable(packingsolver)

add_executable(sheetsolver_oracle main.cpp)
set_target_properties(sheetsolver_oracle PROPERTIES CXX_STANDARD 17)
target_compile_definitions(sheetsolver_oracle PRIVATE
    PACKINGSOLVER_COMMIT="${PACKINGSOLVER_COMMIT}")
target_link_libraries(sheetsolver_oracle PRIVATE
    PackingSolver::rectangleguillotine)

# Reference for the treesearch crate (Stage 2): upstream's beam search on a
# toy knapsack scheme.
add_executable(sheetsolver_treesearch_toy treesearch_toy.cpp)
set_target_properties(sheetsolver_treesearch_toy PROPERTIES CXX_STANDARD 17)
target_link_libraries(sheetsolver_treesearch_toy PRIVATE
    TreeSearchSolver::treesearchsolver)
