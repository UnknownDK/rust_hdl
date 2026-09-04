package types_and_waveforms is
type state_t is (WAIT_FOR_RESET, WAIT_FOR_COMMAND, RECEIVE_HEADER, RECEIVE_PAYLOAD, CHECK_CHECKSUM, SEND_RESPONSE);
type short_t is (IDLE, RUNNING, DONE);
type matrix_t is array (natural range <>, natural range <>, natural range <>) of std_logic_vector(31 downto 0);
type memory_t is array (0 to 255) of byte_t;
end;
entity e is end;
architecture rtl of e is begin
ready <= '1' after 10 ns;
output_signal <= first_long_value after first_long_delay, second_long_value after second_long_delay;
output_signal <= calculate_output(input_data, settings) after PROPAGATION_DELAY;
end;
