/**
 * Reference for the treesearch crate: upstream treesearchsolver's
 * iterative_beam_search_2 on a toy 0-1 knapsack branching scheme.
 *
 * Usage:
 *
 *     sheetsolver_treesearch_toy INSTANCE.json OUTPUT.json
 *         [--minimum-queue-size N] [--growth-factor F]
 *         [--maximum-queue-size N] [--maximum-nodes-expanded N]
 *
 * The same scheme is implemented in Rust in
 * crates/treesearch/tests/toy_knapsack.rs; the Rust search must reproduce
 * this program's output exactly (node counters, best solution, and every
 * new-solution callback). Keep the two in sync.
 *
 * The scheme is chosen to exercise every part of the search: children jump
 * several depths, priorities tie often (broken by node id), some nodes are
 * not comparable, dominance removes queued nodes, and the bound prunes.
 */

#include "treesearchsolver/iterative_beam_search_2.hpp"

#include <nlohmann/json.hpp>

#include <fstream>
#include <iostream>
#include <memory>
#include <string>
#include <vector>

using json = nlohmann::json;
using treesearchsolver::Depth;
using treesearchsolver::NodeId;

namespace
{

struct Item
{
    int64_t weight;
    int64_t profit;
};

struct Instance
{
    int64_t capacity;
    std::vector<Item> items;
};

class ToyKnapsack
{

public:

    struct Node
    {
        std::shared_ptr<Node> parent = nullptr;
        NodeId id = -1;
        /** Items before 'next' have been decided. */
        int64_t next = 0;
        /** Item added by this node, -1 for the root. */
        int64_t item = -1;
        int64_t weight = 0;
        int64_t profit = 0;
    };

    ToyKnapsack(const Instance& instance): instance_(instance) { }

    std::shared_ptr<Node> root() const
    {
        auto node = std::make_shared<Node>();
        node->id = node_id_++;
        return node;
    }

    std::vector<std::shared_ptr<Node>> children(
            const std::shared_ptr<Node>& parent) const
    {
        std::vector<std::shared_ptr<Node>> children;
        for (int64_t j = parent->next; j < (int64_t)instance_.items.size(); ++j) {
            const Item& item = instance_.items[j];
            if (parent->weight + item.weight > instance_.capacity)
                continue;
            auto child = std::make_shared<Node>();
            child->parent = parent;
            child->id = node_id_++;
            child->next = j + 1;
            child->item = j;
            child->weight = parent->weight + item.weight;
            child->profit = parent->profit + item.profit;
            children.push_back(child);
        }
        return children;
    }

    Depth depth(const std::shared_ptr<Node>& node) const { return node->next; }

    bool leaf(const std::shared_ptr<Node>& node) const
    {
        for (int64_t j = node->next; j < (int64_t)instance_.items.size(); ++j)
            if (node->weight + instance_.items[j].weight <= instance_.capacity)
                return false;
        return true;
    }

    bool better(
            const std::shared_ptr<Node>& node_1,
            const std::shared_ptr<Node>& node_2) const
    {
        return node_1->profit > node_2->profit;
    }

    /** True if 'node_1' cannot lead to a solution better than 'node_2'. */
    bool bound(
            const std::shared_ptr<Node>& node_1,
            const std::shared_ptr<Node>& node_2) const
    {
        int64_t upper_bound = node_1->profit;
        for (int64_t j = node_1->next; j < (int64_t)instance_.items.size(); ++j)
            if (node_1->weight + instance_.items[j].weight <= instance_.capacity)
                upper_bound += instance_.items[j].profit;
        return upper_bound <= node_2->profit;
    }

    bool equals(
            const std::shared_ptr<Node>&,
            const std::shared_ptr<Node>&) const
    {
        return false;
    }

    /** Queue order: smaller guide first, ties by creation order. */
    bool operator()(
            const std::shared_ptr<Node>& node_1,
            const std::shared_ptr<Node>& node_2) const
    {
        double guide_1 = -(double)node_1->profit / (node_1->weight + 1);
        double guide_2 = -(double)node_2->profit / (node_2->weight + 1);
        if (guide_1 != guide_2)
            return guide_1 < guide_2;
        return node_1->id < node_2->id;
    }

    bool comparable(const std::shared_ptr<Node>& node) const
    {
        return node->next % 3 != 0;
    }

    struct NodeHasher
    {
        bool operator()(
                const std::shared_ptr<Node>& node_1,
                const std::shared_ptr<Node>& node_2) const
        {
            return node_1->next == node_2->next;
        }

        std::size_t operator()(const std::shared_ptr<Node>& node) const
        {
            return std::hash<int64_t>()(node->next);
        }
    };

    NodeHasher node_hasher() const { return NodeHasher(); }

    bool dominates(
            const std::shared_ptr<Node>& node_1,
            const std::shared_ptr<Node>& node_2) const
    {
        return node_1->weight <= node_2->weight
            && node_1->profit >= node_2->profit;
    }

    std::string display(const std::shared_ptr<Node>& node) const
    {
        return std::to_string(node->profit);
    }

private:

    const Instance& instance_;

    mutable NodeId node_id_ = 0;

};

json node_json(const std::shared_ptr<ToyKnapsack::Node>& node)
{
    std::vector<int64_t> items;
    for (auto n = node; n != nullptr && n->item != -1; n = n->parent)
        items.insert(items.begin(), n->item);
    return {
        {"profit", node->profit},
        {"weight", node->weight},
        {"items", items},
    };
}

}

int main(int argc, char* argv[])
{
    try {
        if (argc < 3) {
            std::cerr << "usage: sheetsolver_treesearch_toy INSTANCE.json OUTPUT.json [options]" << std::endl;
            return 2;
        }
        std::ifstream instance_file(argv[1]);
        json instance_json = json::parse(instance_file);
        Instance instance;
        instance.capacity = instance_json.at("capacity");
        for (const json& item: instance_json.at("items"))
            instance.items.push_back({item.at("weight"), item.at("profit")});

        treesearchsolver::IterativeBeamSearch2Parameters<ToyKnapsack> parameters;
        parameters.verbosity_level = 0;
        for (int i = 3; i + 1 < argc; i += 2) {
            std::string arg = argv[i];
            if (arg == "--minimum-queue-size") {
                parameters.minimum_size_of_the_queue = std::stoll(argv[i + 1]);
            } else if (arg == "--growth-factor") {
                parameters.growth_factor = std::stod(argv[i + 1]);
            } else if (arg == "--maximum-queue-size") {
                parameters.maximum_size_of_the_queue = std::stoll(argv[i + 1]);
            } else if (arg == "--maximum-nodes-expanded") {
                parameters.maximum_number_of_nodes_expanded = std::stoll(argv[i + 1]);
            } else {
                throw std::invalid_argument("unknown argument \"" + arg + "\"");
            }
        }

        json callbacks = json::array();
        parameters.new_solution_callback = [&callbacks](
                const treesearchsolver::Output<ToyKnapsack>& output)
        {
            const auto& ibs_output = static_cast<
                const treesearchsolver::IterativeBeamSearch2Output<ToyKnapsack>&>(output);
            callbacks.push_back({
                {"queue_size", ibs_output.maximum_size_of_the_queue},
                {"optimal", ibs_output.optimal},
                {"best", node_json(ibs_output.solution_pool.best())},
            });
        };

        ToyKnapsack scheme(instance);
        auto output = treesearchsolver::iterative_beam_search_2(scheme, parameters);

        json result = {
            {"parameters", {
                {"minimum_queue_size", parameters.minimum_size_of_the_queue},
                {"growth_factor", parameters.growth_factor},
                {"maximum_queue_size", parameters.maximum_size_of_the_queue},
                {"maximum_nodes_expanded", parameters.maximum_number_of_nodes_expanded},
            }},
            {"best", node_json(output.solution_pool.best())},
            {"optimal", output.optimal},
            {"final_queue_size", output.maximum_size_of_the_queue},
            {"nodes_generated", output.number_of_nodes_generated},
            {"nodes_added", output.number_of_nodes_added},
            {"nodes_processed", output.number_of_nodes_processed},
            {"nodes_expanded", output.number_of_nodes_expanded},
            {"callbacks", callbacks},
        };
        std::ofstream(argv[2]) << result.dump(2) << std::endl;
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "error: " << e.what() << std::endl;
        return 1;
    }
}
