/**
 * sheetsolver oracle: runs upstream packingsolver's guillotine tree search on
 * an instance in sheetsolver's JSON format and writes the solution in
 * sheetsolver's JSON format.
 *
 * Usage:
 *
 *     sheetsolver_oracle INSTANCE.json OUTPUT.json [--algorithm A]
 *         [--queue-size N] [--benchmark R] [--reduce-subproblems]
 *
 * Every algorithm runs in upstream's deterministic single-threaded mode
 * (not-anytime-sequential) with instance reduction disabled, so that the
 * output is reproducible and depends only on the algorithm being ported
 * (see docs/TESTING.md). --algorithm picks it (default: tree_search):
 *
 * - tree_search: `optimize()` with only the tree search enabled, at queue
 *   size N (default 2048).
 * - maximal_spaces: `tree_search_maximal_spaces()` alone, at queue size N
 *   (default 16, upstream's).
 * - hypergraph: `tree_search_hypergraph_infinite_copies()` alone, with
 *   upstream's not-anytime maximum array size.
 * - sequential_single_knapsack, sequential_value_correction: upstream's
 *   `optimize_sequential_single_knapsack` and
 *   `optimize_sequential_value_correction` (optimize.cpp), reproduced here
 *   with one difference: their single-panel subproblems are solved by
 *   `optimize()` with reduction off (upstream leaves it on; see the Stage 5
 *   decision in docs/SCOPE.md), unless --reduce-subproblems, which is
 *   upstream's own behaviour (to measure what the decision costs).
 *   Upstream's default parameters otherwise.
 * - auto: `optimize()` with upstream's automatic algorithm selection
 *   (reduction off). Records the label of every improvement, which names
 *   the algorithm that found it.
 *
 * With --benchmark R, the oracle instead runs upstream's tree search alone
 * (`tree_search()`, without the bounds `optimize()` computes first) R times,
 * prints the fastest run's time in seconds on stdout, and writes the last
 * run's solution. This is the work `guillotine::solve` does, so the two
 * times compare directly (crates/guillotine/benches/).
 *
 * Field mapping (see docs/FIELDS.md): a panel's or part's length is
 * upstream's width (x), its width is upstream's height (y). Lengths are
 * millimetres with at most one decimal in JSON and tenths of a millimetre
 * inside upstream.
 */

#include "packingsolver/rectangleguillotine/algorithm_formatter.hpp"
#include "packingsolver/rectangleguillotine/instance_builder.hpp"
#include "packingsolver/rectangleguillotine/optimize.hpp"
// Internal headers, to run single algorithms.
#include "rectangleguillotine/tree_search.hpp"
#include "rectangleguillotine/tree_search_maximal_spaces.hpp"
#include "rectangleguillotine/tree_search_hypergraph_infinite_copies.hpp"
#include "algorithms/sequential_value_correction.hpp"

#include <chrono>
#include <limits>

#include <cmath>
#include <cstdlib>
#include <iomanip>
#include <fstream>
#include <iostream>
#include <string>

using namespace packingsolver;
using namespace packingsolver::rectangleguillotine;
using json = nlohmann::json;

namespace
{

/** Default of upstream's 'not_anytime_tree_search_queue_size'. */
constexpr NodeId default_queue_size = 2048;

/**
 * Default of upstream's 'not_anytime_tree_search_maximal_spaces_queue_size'.
 */
constexpr NodeId default_maximal_spaces_queue_size = 16;

[[noreturn]] void fail(const std::string& message)
{
    throw std::invalid_argument(message);
}

Objective read_objective(const std::string& name)
{
    if (name == "bin_packing")
        return Objective::BinPacking;
    if (name == "knapsack")
        return Objective::Knapsack;
    if (name == "open_dimension_x")
        return Objective::OpenDimensionX;
    if (name == "open_dimension_y")
        return Objective::OpenDimensionY;
    fail("unsupported objective \"" + name + "\"");
}

/** Our first-cut directions; upstream "vertical" cuts run across the length. */
CutOrientation read_first_cuts(const std::string& name)
{
    if (name == "either")
        return CutOrientation::Any;
    if (name == "along_length")
        return CutOrientation::Horizontal;
    if (name == "across_length")
        return CutOrientation::Vertical;
    fail("unsupported first_cuts \"" + name + "\"");
}

CutType read_cut_type(const std::string& name)
{
    if (name == "non_exact")
        return CutType::NonExact;
    if (name == "exact")
        return CutType::Exact;
    if (name == "homogenous")
        return CutType::Homogenous;
    fail("unsupported cut_type \"" + name + "\"");
}

/** Lengths are stored as whole tenths of a millimetre. */
constexpr Length tenths_per_mm = 10;

/**
 * Read a length given in millimetres with at most one decimal and return it
 * in tenths of a millimetre.
 */
Length read_length(const json& value)
{
    double tenths = value.get<double>() * tenths_per_mm;
    double rounded = std::round(tenths);
    if (std::abs(tenths - rounded) > 1e-6)
        fail("length " + value.dump() + " has more than one decimal");
    return (Length)rounded;
}

/** Write a length in tenths of a millimetre as millimetres. */
json write_length(Length tenths)
{
    if (tenths % tenths_per_mm == 0)
        return tenths / tenths_per_mm;
    return (double)tenths / tenths_per_mm;
}

/**
 * Area in mm² of a rectangle given in tenths of a millimetre: the unit of
 * the default part value and panel cost (docs/ARCHITECTURE.md).
 */
double area_mm2(Length width, Length height)
{
    return (double)width * height / (tenths_per_mm * tenths_per_mm);
}

/** Build the upstream instance and keep the user ids for the output. */
Instance read_instance(
        const json& j,
        std::vector<std::string>& panel_ids,
        std::vector<std::string>& part_ids)
{
    InstanceBuilder builder;
    builder.set_objective(read_objective(j.at("objective")));

    const json& saw = j.at("saw");
    builder.set_cut_thickness(read_length(saw.at("kerf")));
    if (saw.at("stages").is_string()) {
        if (saw.at("stages") != "unlimited")
            fail("stages must be 2, 3 or \"unlimited\"");
        builder.set_number_of_stages_unlimited();
    } else {
        builder.set_number_of_stages(saw.at("stages").get<Counter>());
    }
    builder.set_cut_type(read_cut_type(saw.at("cut_type")));
    // Our default is "either"; upstream's is vertical.
    builder.set_first_stage_orientation(
            read_first_cuts(saw.value("first_cuts", std::string("either"))));

    for (const json& panel: j.at("panels")) {
        Length length = read_length(panel.at("length"));
        Length width = read_length(panel.at("width"));
        BinTypeId id = builder.add_bin_type(length, width);
        // Our "no quantity" means unlimited; upstream's default is 1.
        builder.set_bin_type_copies(id, panel.value("quantity", (BinPos)-1));
        builder.set_bin_type_cost(id, panel.contains("cost")?
                panel.at("cost").get<Profit>(): area_mm2(length, width));
        panel_ids.push_back(panel.at("id"));
    }

    for (const json& part: j.at("parts")) {
        Length length = read_length(part.at("length"));
        Length width = read_length(part.at("width"));
        ItemTypeId id = builder.add_item_type(
                length,
                width,
                part.value("grain", false));
        builder.set_item_type_copies(id, part.at("quantity").get<ItemPos>());
        builder.set_item_type_profit(id, part.contains("value")?
                part.at("value").get<Profit>(): area_mm2(length, width));
        part_ids.push_back(part.at("id"));
    }

    return builder.build();
}

/**
 * With 'leftover_as_offcut', upstream's residual (-3) is written as an
 * offcut: for the copies of a repeated last panel but its very last one.
 */
std::string node_kind(const SolutionNode& node, bool leftover_as_offcut)
{
    // Upstream stores the bin type in the root's item type, so the root
    // must be recognised by its depth before looking at the item type.
    if (node.d == 0 || !node.children.empty())
        return "piece";
    if (node.d == -1)
        return "trim";
    if (node.item_type_id >= 0)
        return "part";
    switch (node.item_type_id) {
    case -1: return "offcut";
    case -2: return "piece";
    case -3: return leftover_as_offcut? "offcut": "leftover";
    case -4: return "defect";
    }
    fail("unknown node type " + std::to_string(node.item_type_id));
}

json write_node(
        const Instance& instance,
        const SolutionBin& bin,
        SolutionNodeId node_id,
        const std::vector<std::string>& part_ids,
        bool leftover_as_offcut)
{
    const SolutionNode& node = bin.nodes[node_id];
    json j;
    std::string kind = node_kind(node, leftover_as_offcut);
    j["kind"] = kind;
    if (kind == "part") {
        const ItemType& item_type = instance.item_type(node.item_type_id);
        j["part_id"] = part_ids[node.item_type_id];
        j["rotated"] = (item_type.rect.w != item_type.rect.h
                && node.r - node.l != item_type.rect.w);
    }
    j["x"] = write_length(node.l);
    j["y"] = write_length(node.b);
    j["length"] = write_length(node.r - node.l);
    j["width"] = write_length(node.t - node.b);
    j["stage"] = node.d;
    j["children"] = json::array();
    for (SolutionNodeId child_id: node.children)
        j["children"].push_back(write_node(instance, bin, child_id, part_ids, leftover_as_offcut));
    return j;
}

json write_solution(
        const Solution& solution,
        const json& oracle,
        const std::vector<std::string>& panel_ids,
        const std::vector<std::string>& part_ids)
{
    const Instance& instance = solution.instance();
    json j;
    j["oracle"] = {
        {"upstream_commit", PACKINGSOLVER_COMMIT},
        {"optimization_mode", "not_anytime_sequential"},
    };
    j["oracle"].update(oracle);
    j["oracle"]["internal_length_unit"] = "0.1 mm";
    // Upstream's own metrics, unchanged, so Stage 1 can compare the
    // checker's metrics against them. Their units are internal: lengths in
    // tenths of a millimetre, areas in hundredths of mm², profit as given
    // (default: area in mm²).
    j["upstream_metrics"] = {
        {"feasible", solution.feasible()},
        {"number_of_items", solution.number_of_items()},
        {"number_of_bins", solution.number_of_bins()},
        {"profit", solution.profit()},
        {"item_area", solution.item_area()},
        {"area", solution.area()},
        {"full_area", solution.full_area()},
        {"waste", solution.waste()},
        {"waste_percentage", solution.waste_percentage()},
        {"width", solution.width()},
        {"height", solution.height()},
    };
    j["patterns"] = json::array();
    for (BinPos bin_pos = 0;
            bin_pos < solution.number_of_different_bins();
            ++bin_pos) {
        const SolutionBin& bin = solution.bin(bin_pos);
        SolutionNodeId root_id = -1;
        for (SolutionNodeId node_id = 0;
                node_id < (SolutionNodeId)bin.nodes.size();
                ++node_id) {
            if (bin.nodes[node_id].f == -1) {
                root_id = node_id;
                break;
            }
        }
        if (root_id == -1)
            fail("bin without a root node");
        // Upstream's leftover only belongs to the very last panel: a last
        // panel cut several times becomes two patterns, the copies but the
        // last with the leftover as an offcut, then the last one.
        bool has_leftover = false;
        for (const SolutionNode& node: bin.nodes)
            if (node.item_type_id == -3 && node.d > 0)
                has_leftover = true;
        bool split = has_leftover && bin.copies > 1;
        auto pattern = [&](BinPos copies, bool leftover_as_offcut)
        {
            return json{
                {"panel_id", panel_ids[bin.bin_type_id]},
                {"repeat", copies},
                // Upstream "vertical" first cuts are at a fixed x: they run
                // across the panel's length (x).
                {"first_cuts",
                    bin.first_cut_orientation == CutOrientation::Vertical?
                    "across_length": "along_length"},
                {"root", write_node(instance, bin, root_id, part_ids, leftover_as_offcut)},
            };
        };
        if (split)
            j["patterns"].push_back(pattern(bin.copies - 1, true));
        j["patterns"].push_back(pattern(split? 1: bin.copies, false));
    }
    return j;
}

/** Parameters every algorithm and subproblem runs with. */
OptimizeParameters deterministic_parameters()
{
    OptimizeParameters parameters;
    parameters.verbosity_level = 0;
    parameters.optimization_mode = OptimizationMode::NotAnytimeSequential;
    parameters.reduction_parameters.reduce = false;
    // The tree search needs no LP solver, but upstream computes a
    // one-dimensional bound with one before searching. The bound can only
    // stop a search early once a solution matches it, which does not
    // change the best solution found (later solutions must be strictly
    // better to replace it); Stage 3 verified it.
    parameters.linear_programming_solver_name
        = columngenerationsolver::SolverName::Highs;
    return parameters;
}

/**
 * The single-panel subproblem solver of the sequential algorithms: as in
 * optimize.cpp, but with reduction off.
 */
/** --reduce-subproblems: upstream's own subproblems, reduced. */
bool reduce_subproblems = false;

SequentialValueCorrectionFunction<Instance, Solution> knapsack_solver(
        NodeId queue_size,
        NodeId queue_size_maximal_spaces)
{
    return [queue_size, queue_size_maximal_spaces](const Instance& kp_instance)
    {
        OptimizeParameters kp_parameters = deterministic_parameters();
        kp_parameters.reduction_parameters.reduce = reduce_subproblems;
        kp_parameters.not_anytime_tree_search_queue_size = queue_size;
        kp_parameters.not_anytime_tree_search_maximal_spaces_queue_size
            = queue_size_maximal_spaces;
        auto pool = optimize(kp_instance, kp_parameters).solution_pool;
        if (std::getenv("SHEETSOLVER_TRACE")) {
            std::cerr << "kp bin " << kp_instance.bin_type(0).rect.w << "x" << kp_instance.bin_type(0).rect.h << " items";
            for (ItemTypeId i = 0; i < kp_instance.number_of_item_types(); ++i)
                std::cerr << " " << kp_instance.item_type(i).rect.w << "x" << kp_instance.item_type(i).rect.h
                    << "*" << kp_instance.item_type(i).copies << "@" << std::setprecision(17) << kp_instance.item_type(i).profit;
            std::cerr << " -> profit " << pool.best().profit() << " items " << pool.best().number_of_items() << " by " << pool.best_label() << std::endl;
        }
        return pool;
    };
}

/**
 * upstream: optimize.cpp, 'optimize_sequential_single_knapsack' in
 * not-anytime mode (one pass).
 */
Solution sequential_single_knapsack(const Instance& instance, json& oracle)
{
    OptimizeParameters defaults;
    NodeId queue_size
        = defaults.not_anytime_sequential_single_knapsack_subproblem_tree_search_queue_size;
    NodeId queue_size_maximal_spaces
        = defaults.not_anytime_sequential_single_knapsack_subproblem_tree_search_maximal_spaces_queue_size;
    SequentialValueCorrectionParameters<Instance, Solution, rectangleguillotine::Output> svc_parameters;
    svc_parameters.verbosity_level = 0;
    svc_parameters.maximum_number_of_iterations = 1;
    svc_parameters.initial_profit_exponent = 1.0;
    auto output = packingsolver::sequential_value_correction<Instance, InstanceBuilder, Solution, AlgorithmFormatter, rectangleguillotine::Output>(
            instance,
            knapsack_solver(queue_size, queue_size_maximal_spaces),
            svc_parameters);
    oracle["subproblem_queue_size"] = queue_size;
    oracle["subproblem_maximal_spaces_queue_size"] = queue_size_maximal_spaces;
    oracle["subproblem_reduction"] = reduce_subproblems;
    return output.solution_pool.best();
}

/**
 * upstream: optimize.cpp, 'optimize_sequential_value_correction' in
 * not-anytime mode.
 */
Solution sequential_value_correction(const Instance& instance, json& oracle)
{
    OptimizeParameters defaults;
    NodeId queue_size
        = defaults.sequential_value_correction_subproblem_tree_search_queue_size;
    NodeId queue_size_maximal_spaces
        = defaults.sequential_value_correction_subproblem_tree_search_maximal_spaces_queue_size;
    Counter iterations
        = defaults.not_anytime_sequential_value_correction_number_of_iterations;
    SequentialValueCorrectionParameters<Instance, Solution, rectangleguillotine::Output> svc_parameters;
    svc_parameters.verbosity_level = 0;
    svc_parameters.initial_profit_exponent = 1.1;
    svc_parameters.maximum_number_of_iterations = iterations;
    auto output = packingsolver::sequential_value_correction<Instance, InstanceBuilder, Solution, AlgorithmFormatter, rectangleguillotine::Output>(
            instance,
            knapsack_solver(queue_size, queue_size_maximal_spaces),
            svc_parameters);
    oracle["subproblem_queue_size"] = queue_size;
    oracle["subproblem_maximal_spaces_queue_size"] = queue_size_maximal_spaces;
    oracle["subproblem_reduction"] = reduce_subproblems;
    oracle["iterations"] = iterations;
    return output.solution_pool.best();
}

/** Runs one algorithm as the usage above describes. */
Solution run_algorithm(
        const Instance& instance,
        const std::string& algorithm,
        NodeId queue_size,
        json& oracle)
{
    oracle["algorithm"] = algorithm;
    if (algorithm == "tree_search") {
        OptimizeParameters parameters = deterministic_parameters();
        parameters.use_tree_search = true;
        parameters.not_anytime_tree_search_queue_size = queue_size;
        oracle["queue_size"] = queue_size;
        return optimize(instance, parameters).solution_pool.best();
    }
    if (algorithm == "maximal_spaces") {
        TreeSearchMaximalSpacesParameters parameters;
        parameters.verbosity_level = 0;
        parameters.optimization_mode = OptimizationMode::NotAnytimeSequential;
        parameters.not_anytime_tree_search_queue_size = queue_size;
        oracle["queue_size"] = queue_size;
        return tree_search_maximal_spaces(instance, parameters).solution_pool.best();
    }
    if (algorithm == "hypergraph") {
        TreeSearchHypergraphInfiniteCopiesParameters parameters;
        parameters.verbosity_level = 0;
        parameters.maximum_array_size = OptimizeParameters()
            .not_anytime_tree_search_hypergraph_infinite_copies_maximum_array_size;
        oracle["maximum_array_size"] = parameters.maximum_array_size;
        return tree_search_hypergraph_infinite_copies(instance, parameters).solution_pool.best();
    }
    if (algorithm == "sequential_single_knapsack")
        return sequential_single_knapsack(instance, oracle);
    if (algorithm == "sequential_value_correction")
        return sequential_value_correction(instance, oracle);
    if (algorithm == "auto") {
        OptimizeParameters parameters = deterministic_parameters();
        json labels = json::array();
        // The callback also fires on bound updates, with the current best
        // label: only new labels are improvements.
        parameters.new_solution_callback = [&labels](const rectangleguillotine::Output& output)
        {
            const std::string& label = output.solution_pool.best_label();
            if (!label.empty() && (labels.empty() || labels.back() != label))
                labels.push_back(label);
        };
        Solution solution = optimize(instance, parameters).solution_pool.best();
        oracle["improvements"] = labels;
        return solution;
    }
    fail("unknown algorithm \"" + algorithm + "\"");
}

}

int main(int argc, char* argv[])
{
    try {
        if (argc < 3) {
            std::cerr << "usage: sheetsolver_oracle INSTANCE.json OUTPUT.json"
                " [--algorithm A] [--queue-size N] [--benchmark R]" << std::endl;
            return 2;
        }
        std::string instance_path = argv[1];
        std::string output_path = argv[2];
        std::string algorithm = "tree_search";
        NodeId queue_size = -1;
        int benchmark_repeats = 0;
        for (int i = 3; i < argc; ++i) {
            std::string arg = argv[i];
            if (arg == "--queue-size" && i + 1 < argc) {
                queue_size = std::stoll(argv[++i]);
            } else if (arg == "--algorithm" && i + 1 < argc) {
                algorithm = argv[++i];
            } else if (arg == "--reduce-subproblems") {
                reduce_subproblems = true;
            } else if (arg == "--benchmark" && i + 1 < argc) {
                benchmark_repeats = std::stoi(argv[++i]);
            } else {
                fail("unknown argument \"" + arg + "\"");
            }
        }
        if (queue_size == -1) {
            queue_size = (algorithm == "maximal_spaces")?
                default_maximal_spaces_queue_size:
                default_queue_size;
        }

        std::ifstream instance_file(instance_path);
        if (!instance_file.good())
            fail("unable to open \"" + instance_path + "\"");
        json instance_json = json::parse(instance_file);

        std::vector<std::string> panel_ids;
        std::vector<std::string> part_ids;
        Instance instance = read_instance(instance_json, panel_ids, part_ids);

        Solution solution(instance);
        json oracle;
        if (benchmark_repeats > 0) {
            // Times the algorithm alone: no bounds, no JSON.
            double fastest = std::numeric_limits<double>::infinity();
            for (int repeat = 0; repeat < benchmark_repeats; ++repeat) {
                auto start = std::chrono::steady_clock::now();
                if (algorithm == "tree_search") {
                    TreeSearchParameters ts_parameters;
                    ts_parameters.verbosity_level = 0;
                    ts_parameters.optimization_mode = OptimizationMode::NotAnytimeSequential;
                    ts_parameters.not_anytime_tree_search_queue_size = queue_size;
                    solution = tree_search(instance, ts_parameters).solution_pool.best();
                    oracle = {{"algorithm", algorithm}, {"queue_size", queue_size}};
                } else {
                    oracle = json::object();
                    solution = run_algorithm(instance, algorithm, queue_size, oracle);
                }
                std::chrono::duration<double> elapsed
                    = std::chrono::steady_clock::now() - start;
                fastest = std::min(fastest, elapsed.count());
            }
            std::cout << fastest << std::endl;
        } else {
            solution = run_algorithm(instance, algorithm, queue_size, oracle);
        }

        std::ofstream output_file(output_path);
        if (!output_file.good())
            fail("unable to open \"" + output_path + "\"");
        output_file << write_solution(solution, oracle, panel_ids, part_ids).dump(2)
            << std::endl;
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "error: " << e.what() << std::endl;
        return 1;
    }
}
