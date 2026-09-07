# Error handling: SPEC.md §12, row by row

Every row of the specified error matrix, what the code actually does about it, and the
test that holds it there. Written because "handled" is not a claim anybody can check —
a named test is.

The last column is the honest one. Where a row says the *policy* is tested but the
*wiring* is not, that is because the call site is a Tauri command, which cannot be
invoked without a running app; the decision it makes was extracted into a plain function
so that at least the interesting half is covered. Those are the rows to look at first
when something behaves oddly in the app but passes in CI.

| # | Situation | What happens | Where | Test |
|---|---|---|---|---|
| 1 | Insufficient disk space | Refused before a download starts and before a build starts, reporting need and available | `cache::download_need`, `cache::precheck_space`, `pipeline::required_bytes` | `a_download_is_measured_by_what_it_leaves_on_disk_not_what_crosses_the_wire`, `reports_free_space_and_rejects_impossible_requests`, `a_build_refuses_to_start_when_the_disk_is_too_small`, `disk_space_reports_both_numbers_and_what_to_do` |
| 2 | Download interrupted | Retried with exponential backoff, then resumed from the byte it stopped at | `download::download`, `download::download_zip_member_inflated` | `resumes_mid_stream_without_restarting_the_transfer` |
| 3 | Checksum mismatch | Discarded, nothing published, retry offered | `download`, `stac::Digest::verify` | `rejects_a_corrupt_archive_and_leaves_nothing_behind`, `inflates_while_downloading_and_verifies_the_archive_checksum`, `a_downloaded_file_that_fails_its_checksum_is_not_used` |
| 4 | STAC API unreachable | Falls back to the last answer the API gave, marked stale and dated | `stac::latest_cached` | `an_unreachable_api_falls_back_to_the_last_answer_and_says_it_is_stale`, `an_unreachable_api_with_no_cached_answer_reports_the_connection_failure`, `a_corrupt_cached_answer_is_not_used`, `a_new_release_replaces_the_remembered_one` |
| 5 | swissALTI3D tile missing | Skipped; the build report names the cells, capped, sorted | `pipeline::missing_tile_warning`, `elevation::Cell::label` | `the_warning_names_the_cells_swisstopo_names`, `a_long_list_is_capped_but_still_reports_the_true_count` |
| 6 | Estimate exceeds device limit | Only the remedies that would change this recipe, ordered by what they save; start blocked when no split fits | `estimate::budget_verdict`, `partition::plan` | `the_remedies_are_ordered_by_what_they_save`, `remedies_that_would_change_nothing_are_not_offered`, `a_map_exactly_at_the_budget_fits`, `a_budget_smaller_than_one_map_stops_rather_than_looping` |
| 7 | Tile count exceeds device limit | `--max-nodes` doubled and the split retried, to splitter's own documented ceiling; then map sets | `garmin::retune_max_nodes`, `pipeline` split loop | `too_many_tiles_doubles_the_node_budget`, `retuning_stops_at_the_ceiling_rather_than_doubling_past_it`, `the_retune_sequence_terminates`, `an_unfittable_tile_count_suggests_map_sets_not_denser_tiles` |
| 8 | Java tool non-zero exit | Stage, exit code, the command line as run, the stderr tail, and a plain-language cause where recognised | `garmin::run`, `garmin::describe`, `diagnose` | `a_failure_reports_the_stage_the_exit_code_and_the_command_line`, `the_reported_command_line_is_pasteable`, `a_java_heap_failure_is_explained_in_terms_of_the_build` |
| 9 | Device unplugged mid-copy | Aborted; partial removed and any backup restored; when cleanup itself fails, the stranded file is named | `install::install_with` | `an_unplugged_device_reports_the_incomplete_file_by_name`, `a_failed_copy_that_could_be_cleaned_up_says_retrying_is_safe`, `a_failed_copy_restores_the_map_that_was_already_there` |
| 10 | Target file already exists | Reported before writing; backup on request; never silently replaced without being told to | `install::install`, `ipc::plan_install` | `an_existing_map_is_kept_when_a_backup_was_asked_for`, `without_a_backup_the_existing_map_is_replaced`, `the_temporary_names_are_appended_not_substituted` |
| 11 | Corrupt cache detected | Read verified on open; damaged dataset moved to `.quarantine`, never deleted, never reused | `pipeline::open_verified`, `cache::quarantine_containing` | `a_build_quarantines_a_damaged_geopackage_instead_of_failing_obscurely`, `a_corrupt_file_quarantines_the_dataset_it_belongs_to`, `a_file_outside_the_cache_is_left_alone` |
| 12 | App killed mid-build | Marker plus dead owner identifies an orphan; strays found by their own command lines; resume or discard | `recovery` | `a_build_whose_process_is_gone_is_reported_with_what_it_costs`, `java_children_of_an_abandoned_build_are_found_by_their_command_line`, `a_reused_pid_belonging_to_another_program_does_not_protect_a_build`, and eleven more |

## Two rows that needed a decision

**Row 6, "block".** A hard block on the size estimate would refuse builds that fit: the
estimate's own accuracy target is ±25 % (FR-60), so a map predicted at 105 % of budget is
as likely to fit as not. What is blocked instead is the case that is *certainly*
impossible — the partition planner reporting that no division of this area fits this
device — and being over budget produces the remedy list plus a split plan. Recorded in
SPEC.md §12 rather than left as a silent reinterpretation.

**Row 7, the ceiling.** `--max-nodes` is retuned upward to 1,600,000 and no further,
because that is splitter r654's own documented default and therefore the largest value
its author treats as ordinary. Going beyond it to satisfy a tile budget would trade a
limit that is documented for one we would be guessing at, which rule 3 of the working
agreement forbids.

## What is not covered

* The **call sites** in `src-tauri/src/ipc.rs` for rows 1 and 10 — `acquire_dataset`
  calling the precheck, `install_map` calling the installer. The functions they call are
  tested; that they call them is not.
* Row 2's **backoff schedule** is asserted only as "a retry happened", not as the
  interval sequence.
* Rows 9 and 12 are tested against simulated failures — an injected copy error, a fake
  process list. A real cable pulled from a real Edge is check C.6 of
  [device-verification.md](device-verification.md) and has not been done.
