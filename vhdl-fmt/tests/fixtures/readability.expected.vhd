library ieee;
    use ieee.std_logic_1164.all;

entity example is
    port (
        input_valid: in std_logic;
        output_ready: in std_logic;
        transfer_enabled: in std_logic
    );
end;

architecture rtl of example is
    type header_t is record
        valid: std_logic;
        opcode, length: natural;
    end record;
    constant OP_WRITE: natural := 1;

    function payload_size return natural is
    begin
        return 16;
    end;

    signal header: header_t;
begin
    process(all)
    begin
        if input_valid = '1'
            and output_ready = '1'
            and transfer_enabled = '1'
        then
            header <= (
                valid  => '1',
                opcode => OP_WRITE,
                length => payload_size
            );
        else
            header <= (
                valid  => '0',
                opcode => 0,
                length => 0
            );
        end if;
    end process;

    -- Monitor the generated header.
    process(header)
    begin
        assert header.length <= 16
            report "Invalid length"
            severity failure;
    end process;
end;
