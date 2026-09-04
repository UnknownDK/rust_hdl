package p is
    attribute mark_debug: boolean;
    attribute mark_debug of
        very_long_internal_signal_name: signal is
        true;
    alias received_data_word:
        std_logic_vector(31 downto 0) is
        very_long_internal_data_bus(31 downto 0);
    alias debug_data is
        <<
            signal .tb.dut.internal_data_bus :
                std_logic_vector(31 downto 0)
        >>;
end;

entity e is
    port (
        data         : in std_logic_vector(31 downto 0);
        output_ready : out boolean
    );
end;

architecture rtl of e is
begin
    u: entity work.child
        port map (
            data         => input_data_bus,
            output_ready => output_ready_signal
        );
end;
