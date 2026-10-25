/**
 * sheetsolver oracle: runs upstream packingsolver's guillotine tree search on
 * an instance in sheetsolver's JSON format and writes the solution in
 * sheetsolver's JSON format.
 *
 * Usage:
 *
 *     sheetsolver_oracle INSTANCE.json OUTPUT.json [--queue-size N]
 *
 * The search runs in upstream's deterministic single-threaded mode
 * (not-anytime-sequential) with a fixed queue size, with only the tree
 * search enabled and instance reduction disabled, so that the output is
 * reproducible and depends only on the algorithm being ported (see
 * docs/TESTING.md).
 *
 * Field mapping (see docs/FIELDS.md): a panel's or part's length is
 * upstream's width (x), its width is upstream's height (y). Lengths are
 * millimetres with at most one decimal in JSON and tenths of a millimetre
 * inside upstream.
 */

#include "packingsolver/rectangleguillotine/instance_builder.hpp"
#include "packingsolver/rectangleguillotine/optimize.hpp"

#include <cmath>
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

std::string node_kind(const SolutionNode& node)
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
    case -3: return "leftover";
    case -4: return "defect";
    }
    fail("unknown node type " + std::to_string(node.item_type_id));
}

json write_node(
        const Instance& instance,
        const SolutionBin& bin,
        SolutionNodeId node_id,
        const std::vector<std::string>& part_ids)
{
    const SolutionNode& node = bin.nodes[node_id];
    json j;
    std::string kind = node_kind(node);
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
        j["children"].push_back(write_node(instance, bin, child_id, part_ids));
    return j;
}

json write_solution(
        const Solution& solution,
        NodeId queue_size,
        const std::vector<std::string>& panel_ids,
        const std::vector<std::string>& part_ids)
{
    const Instance& instance = solution.instance();
    json j;
    j["oracle"] = {
        {"upstream_commit", PACKINGSOLVER_COMMIT},
        {"optimization_mode", "not_anytime_sequential"},
        {"algorithm", "tree_search"},
        {"queue_size", queue_size},
        {"internal_length_unit", "0.1 mm"},
    };
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
        j["patterns"].push_back({
            {"panel_id", panel_ids[bin.bin_type_id]},
            {"repeat", bin.copies},
            // Upstream "vertical" first cuts are at a fixed x: they run
            // across the panel's length (x).
            {"first_cuts",
                bin.first_cut_orientation == CutOrientation::Vertical?
                "across_length": "along_length"},
            {"root", write_node(instance, bin, root_id, part_ids)},
        });
    }
    return j;
}

}

int main(int argc, char* argv[])
{
    try {
        if (argc < 3) {
            std::cerr << "usage: sheetsolver_oracle INSTANCE.json OUTPUT.json"
                " [--queue-size N]" << std::endl;
            return 2;
        }
        std::string instance_path = argv[1];
        std::string output_path = argv[2];
        NodeId queue_size = default_queue_size;
        for (int i = 3; i < argc; ++i) {
            std::string arg = argv[i];
            if (arg == "--queue-size" && i + 1 < argc) {
                queue_size = std::stoll(argv[++i]);
            } else {
                fail("unknown argument \"" + arg + "\"");
            }
        }

        std::ifstream instance_file(instance_path);
        if (!instance_file.good())
            fail("unable to open \"" + instance_path + "\"");
        json instance_json = json::parse(instance_file);

        std::vector<std::string> panel_ids;
        std::vector<std::string> part_ids;
        Instance instance = read_instance(instance_json, panel_ids, part_ids);

        OptimizeParameters parameters;
        parameters.verbosity_level = 0;
        parameters.optimization_mode = OptimizationMode::NotAnytimeSequential;
        parameters.reduction_parameters.reduce = false;
        parameters.use_tree_search = true;
        parameters.not_anytime_tree_search_queue_size = queue_size;
        // The tree search needs no LP solver, but upstream computes a
        // one-dimensional bound with one before searching. The bound can
        // only stop the search early once a solution matches it. We assume
        // this does not change the best solution found (later solutions
        // must be strictly better to replace it); Stage 3 verifies it.
        parameters.linear_programming_solver_name
            = columngenerationsolver::SolverName::Highs;
        const rectangleguillotine::Output output = optimize(instance, parameters);

        json solution_json = write_solution(
                output.solution_pool.best(),
                queue_size,
                panel_ids,
                part_ids);
        std::ofstream output_file(output_path);
        if (!output_file.good())
            fail("unable to open \"" + output_path + "\"");
        output_file << solution_json.dump(2) << std::endl;
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "error: " << e.what() << std::endl;
        return 1;
    }
}
