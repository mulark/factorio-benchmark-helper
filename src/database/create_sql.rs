pub const CREATE_SQL: &str = "
BEGIN;
CREATE TABLE IF NOT EXISTS `benchmark_collection` (
  `collection_id` integer NOT NULL PRIMARY KEY AUTOINCREMENT
,  `pattern` varchar(100)  NULL
,  `factorio_version` varchar(10)  NOT NULL
,  `ticks` integer  NOT NULL
,  `platform` varchar(100)  NOT NULL
,  `executable_type` varchar(100)  NOT NULL
,  `number_of_mods_installed` integer  NOT NULL
,  `notes` text  NULL
,  `kernel_version` text  NULL
);

CREATE TABLE IF NOT EXISTS `benchmark_base` (
  `benchmark_id` integer NOT NULL PRIMARY KEY AUTOINCREMENT
,  `map_name` varchar(100)  NOT NULL
,  `saved_map_version` varchar(10)  NOT NULL
,  `number_of_runs` integer  NOT NULL CHECK (`number_of_runs` > 0)
,  `ticks` integer  NOT NULL
,  `collection_id` integer  NOT NULL
,  `map_hash` varchar(40)  NOT NULL
,  CONSTRAINT `benchmark_base_ibfk_1` FOREIGN KEY (`collection_id`) REFERENCES `benchmark_collection` (`collection_id`)
,  CONSTRAINT `hash_length_check` CHECK (length(`map_hash`) = 40)
);
CREATE INDEX 'idx_benchmark_base_collection_id' ON 'benchmark_base' (`collection_id`);

CREATE TABLE IF NOT EXISTS 'benchmark_verbose' (
'unused_row_index' integer PRIMARY KEY AUTOINCREMENT
,  `run_index` integer NOT NULL
,  `tick_number` integer NOT NULL
,  `wholeUpdate` integer  NOT NULL
,  `gameUpdate` integer  NOT NULL
,  `circuitNetworkUpdate` integer   NOT NULL
,  `transportLinesUpdate` integer   NOT NULL
,  `fluidsUpdate` integer   NOT NULL
,  `entityUpdate` integer   NOT NULL
,  `mapGenerator` integer   NOT NULL
,  `electricNetworkUpdate` integer   NOT NULL
,  `logisticManagerUpdate` integer   NOT NULL
,  `constructionManagerUpdate` integer   NOT NULL
,  `pathFinder` integer   NOT NULL
,  `trains` integer   NOT NULL
,  `trainPathFinder` integer   NOT NULL
,  `commander` integer   NOT NULL
,  `chartRefresh` integer   NOT NULL
,  `luaGarbageIncremental` integer   NOT NULL
,  `chartUpdate` integer   NOT NULL
,  `scriptUpdate` integer   NOT NULL
,  `benchmark_id` integer  NOT NULL
,  CONSTRAINT `benchmark_verbose_ibfk_1` FOREIGN KEY (`benchmark_id`) REFERENCES `benchmark_base` (`benchmark_id`)
);
COMMIT;
";
