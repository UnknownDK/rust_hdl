entity layout_rules is port(first_input_signal, second_input_signal, third_input_signal: in std_logic_vector(31 downto 0)); end;
architecture rtl of layout_rules is begin
g: if FEATURE_A_ENABLED and FEATURE_B_ENABLED and FEATURE_C_ENABLED generate
with selection_value select result <= first_value when first_choice, second_value when second_choice, default_value when others;
end generate;
process begin
while first_condition and second_condition and third_condition loop null; end loop;
case selected_mode is when MODE_A | MODE_B | MODE_C | MODE_D | MODE_E => null; when others => null; end case;
wait;
end process;
end;
